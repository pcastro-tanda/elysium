# frozen_string_literal: true

# Creates a compiling, registered skeleton for each named RuboCop cop so a
# porter only has to fill in one file (see docs/porting/KIT.md):
#
#   * <crate>/src/<dept>/<snake>.rs with a `RuleMeta` pre-filled from the
#     cop's config/default.yml entry (name, summary, enabled, severity,
#     `Stability::Nursery`) and an empty `configure`;
#   * `pub mod <snake>;` in <crate>/src/<dept>/mod.rs;
#   * a line in the `rule_set! {}` of <crate>/src/lib.rs.
#
# <crate> is crates/rules for core RuboCop cops (defaults from
# --rubocop-src's config/default.yml) and crates/rules_<key> for a cop of an
# extension gem in tools/extension_gems.rb (Rails/, Performance/,
# ThreadSafety/, Minitest/, Sorbet/; defaults from the gem's vendored
# default.yml in that crate, so no gem source is needed).
#
# Existing files and registrations are left alone, so it is safe to re-run.
# Fixtures are still produced separately by tools/port_spec.rb (ADR 0006).
#
#   ruby tools/scaffold_cop.rb --rubocop-src /path/to/rubocop-1.91.0 Lint/EmptyWhen Style/Dir
#   ruby tools/scaffold_cop.rb Rails/ApplicationRecord Minitest/AssertNil
#
# `--gem-src KEY=PATH` (as for tools/port_spec.rb) only refines the upstream
# path quoted in the skeleton's doc comment (rubocop-sorbet nests some cops a
# directory deeper); without it the department directory is assumed.

require 'fileutils'
require 'optparse'
require 'yaml'
require_relative 'extension_gems'

ROOT = File.expand_path('..', __dir__)
CORE_RULES = File.join(ROOT, 'crates/rules/src')

RUST_KEYWORDS = %w[
  as break const continue crate else enum extern false fn for if impl in let loop match mod
  move mut pub ref return self static struct super trait true type unsafe use where while
  async await dyn abstract become box do final macro override priv typeof unsized virtual
  yield try gen
].freeze

def rust_str(text)
  text.to_s.gsub('\\', '\\\\\\\\').gsub('"', '\"')
end

def skeleton(cop, entry, origin)
  dept, name = cop.split('/')
  default = dept == 'Lint' ? 'warning' : 'convention'
  severity = entry.fetch('Severity', default).capitalize
  <<~RUST
    //! `#{cop}`, ported from #{origin}.

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

def register_mod(dept_dir, dept, mod_name)
  path = File.join(dept_dir, 'mod.rs')
  text = File.exist?(path) ? File.read(path) : "//! `#{dept}` department.\n"
  return if text.include?("pub mod #{mod_name};")

  text = "#{text}\n" unless text.end_with?("\n")
  # The first module after the doc comment needs a blank line between them.
  text = "#{text}\n" unless text.include?('pub mod ')
  File.write(path, "#{text}pub mod #{mod_name};\n")
end

def register_rule_set(rules_src, line)
  path = File.join(rules_src, 'lib.rs')
  text = File.read(path)
  return if text.include?("    #{line},\n")

  text = text.sub('rule_set! {}', "rule_set! {\n}")
  File.write(path, text.sub("rule_set! {\n", "rule_set! {\n    #{line},\n"))
end

rubocop_src = nil
gem_src = {}
OptionParser.new do |opts|
  opts.banner = 'usage: scaffold_cop.rb [--rubocop-src DIR] [--gem-src KEY=PATH,...] Dept/Cop...'
  opts.on('--rubocop-src DIR', 'RuboCop source checkout (needed for core cops)') { |dir| rubocop_src = dir }
  opts.on('--gem-src KEY=PATH', 'Extension gem source checkout(s), for the upstream path only') do |v|
    gem_src.merge!(ExtensionGems.parse_sources(v))
  end
end.parse!
abort 'no cops given' if ARGV.empty?

defaults = Hash.new do |cache, path|
  cache[path] = YAML.unsafe_load_file(path)
end

ARGV.each do |cop|
  dept, name = cop.split('/')
  abort "expected Dept/Cop, got #{cop.inspect}" unless dept && name

  extension = ExtensionGems.for_department(dept)
  dept_snake = ExtensionGems.snake(dept)
  mod_name = ExtensionGems.snake(name)
  if extension
    entry = defaults[extension.vendored_default_yml].fetch(cop) do
      abort "#{cop}: not in #{extension.vendored_default_yml.delete_prefix("#{ROOT}/")}"
    end
    rules_src = extension.crate_src
    src = rubocop_src ? extension.source(gem_src, rubocop_src) : gem_src[extension.key]
    upstream = src && Dir.glob(File.join(src, 'lib/rubocop/cop', dept_snake, '**', "#{mod_name}.rb")).first
    upstream = upstream ? upstream.delete_prefix("#{File.expand_path(src)}/") : "lib/rubocop/cop/#{dept_snake}/#{mod_name}.rb"
    origin = "#{extension.gem}'s\n//! `#{upstream}`"
  else
    abort "#{cop}: core cops need --rubocop-src" unless rubocop_src
    entry = defaults[File.join(rubocop_src, 'config/default.yml')].fetch(cop) { abort "#{cop}: not in default.yml" }
    rules_src = CORE_RULES
    origin = "RuboCop's\n//! `lib/rubocop/cop/#{dept_snake}/#{mod_name}.rb`"
  end
  dept_dir = File.join(rules_src, dept_snake)
  file = File.join(dept_dir, "#{mod_name}.rs")

  unless File.exist?(file)
    FileUtils.mkdir_p(dept_dir)
    File.write(file, skeleton(cop, entry, origin))
    puts "created #{file.delete_prefix("#{ROOT}/")}"
  end
  # Rust keywords (`Lint/Loop` -> `loop`) need raw identifiers in paths.
  ident = RUST_KEYWORDS.include?(mod_name) ? "r##{mod_name}" : mod_name
  register_mod(dept_dir, dept, ident)
  register_rule_set(rules_src, "#{dept_snake}::#{ident}::#{name}")
end
