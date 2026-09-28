# frozen_string_literal: true

# Creates a compiling, registered skeleton for each named RuboCop cop so a
# porter only has to fill in one file (see docs/porting/KIT.md):
#
#   * crates/rules/src/<dept>/<snake>.rs with a `RuleMeta` pre-filled from
#     RuboCop's config/default.yml (name, summary, enabled, severity,
#     `Stability::Nursery`) and an empty `configure`;
#   * `pub mod <snake>;` in crates/rules/src/<dept>/mod.rs;
#   * a line in the `rule_set! {}` of crates/rules/src/lib.rs.
#
# Existing files and registrations are left alone, so it is safe to re-run.
# Fixtures are still produced separately by tools/port_spec.rb (ADR 0006).
#
#   ruby tools/scaffold_cop.rb --rubocop-src /path/to/rubocop-1.82.1 Lint/EmptyWhen Style/Dir

require 'optparse'
require 'yaml'

ROOT = File.expand_path('..', __dir__)
RULES = File.join(ROOT, 'crates/rules/src')

def snake(name)
  name.gsub(/([A-Z]+)([A-Z][a-z])/, '\1_\2').gsub(/([a-z\d])([A-Z])/, '\1_\2').downcase
end

def rust_str(text)
  text.to_s.gsub('\\', '\\\\\\\\').gsub('"', '\"')
end

def skeleton(cop, entry, rubocop_path)
  dept, name = cop.split('/')
  severity = dept == 'Lint' ? 'Warning' : 'Convention'
  <<~RUST
    //! `#{cop}`, ported from RuboCop's
    //! `#{rubocop_path}`.

    use linter::{
        Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
        Stability,
    };
    use ruby_ast::{Node, NodeKind};

    /// #{entry['Description']}
    #[derive(Debug, Clone)]
    pub struct #{name};

    impl Rule for #{name} {
        const META: RuleMeta = RuleMeta {
            name: "#{cop}",
            department: Department::#{dept},
            summary: "#{rust_str(entry['Description'])}",
            explanation: "",
            enabled_by_default: #{entry['Enabled'] == true},
            severity: Severity::#{severity},
            fix: FixAvailability::None,
            stability: Stability::Nursery,
            kinds: &[],
            config: &[],
            blind_spots: "",
        };

        fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
            Ok(Self)
        }

        fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
            let _ = (node, ctx, NodeKind::CallNode);
        }
    }
  RUST
end

def register_mod(dept_dir, mod_name)
  path = File.join(dept_dir, 'mod.rs')
  text = File.read(path)
  return if text.include?("pub mod #{mod_name};")

  File.write(path, text.end_with?("\n") ? "#{text}pub mod #{mod_name};\n" : "#{text}\npub mod #{mod_name};\n")
end

def register_rule_set(line)
  path = File.join(RULES, 'lib.rs')
  text = File.read(path)
  return if text.include?("    #{line},\n")

  File.write(path, text.sub("rule_set! {\n", "rule_set! {\n    #{line},\n"))
end

rubocop_src = nil
OptionParser.new do |opts|
  opts.banner = 'usage: scaffold_cop.rb --rubocop-src DIR Dept/Cop...'
  opts.on('--rubocop-src DIR') { |dir| rubocop_src = dir }
end.parse!
abort 'missing --rubocop-src' unless rubocop_src
abort 'no cops given' if ARGV.empty?

defaults = YAML.load_file(File.join(rubocop_src, 'config/default.yml'))

ARGV.each do |cop|
  entry = defaults.fetch(cop) { abort "#{cop}: not in default.yml" }
  dept, name = cop.split('/')
  dept_snake = snake(dept)
  mod_name = snake(name)
  dept_dir = File.join(RULES, dept_snake)
  file = File.join(dept_dir, "#{mod_name}.rs")
  rubocop_path = "lib/rubocop/cop/#{dept_snake}/#{mod_name}.rb"

  unless File.exist?(file)
    File.write(file, skeleton(cop, entry, rubocop_path))
    puts "created #{file.delete_prefix("#{ROOT}/")}"
  end
  register_mod(dept_dir, mod_name)
  register_rule_set("#{dept_snake}::#{mod_name}::#{name}")
end
