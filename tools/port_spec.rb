#!/usr/bin/env ruby
# frozen_string_literal: true

# Development tool only — never a runtime dependency of the linter/rules crates.
#
# Ports a RuboCop cop's RSpec examples into elysium's fixture format (phase3
# contract, section 3): one <case>.rb per expect_offense/expect_no_offenses
# example, <case>.fixed.rb for expect_correction, an empty <case>.nofix for
# expect_no_corrections, an empty <case>.singlepass when the spec used
# expect_correction(..., loop: false), and <case>.yml for any cop_config /
# other_cops / ruby_version override (else RuboCop defaults apply).
#
# It works by actually running the real RuboCop spec against the real cop,
# with expect_offense/expect_correction/expect_no_offenses/expect_no_corrections
# monkeypatched to record their exact (already-interpolated) arguments before
# delegating to RuboCop's own implementation. Every ported fixture is therefore
# taken verbatim from a passing assertion against the real cop — not a manual
# transcription.
#
# Usage:
#   ruby tools/port_spec.rb --cop Style/TrailingCommaInArguments \
#     --rubocop-src /Users/paulo/Work/lab/corpus/rubocop-1.82.1 \
#     --out crates/rules/fixtures
#
# Requires: the exact RuboCop version pinned by --rubocop-src's
# lib/rubocop/version.rb, and `rspec`, already installed as system gems
# (checked with `gem list`). No network access and no modification of
# --rubocop-src are performed; a scratch Gemfile is written under a temp
# directory and resolved with `bundle lock --local`.

require 'optparse'
require 'fileutils'
require 'tmpdir'
require 'json'
require 'set'

options = { out: 'crates/rules/fixtures' }
OptionParser.new do |opts|
  opts.banner = 'Usage: port_spec.rb --cop Dept/Name --rubocop-src PATH [--out DIR]'
  opts.on('--cop COP', 'Cop name, e.g. Style/TrailingCommaInArguments') { |v| options[:cop] = v }
  opts.on('--rubocop-src PATH', 'Path to a full RuboCop source checkout (has lib/ and spec/)') { |v| options[:rubocop_src] = v }
  opts.on('--out DIR', 'Fixture root (default crates/rules/fixtures)') { |v| options[:out] = v }
end.parse!

abort 'missing --cop Dept/Name' unless options[:cop]
abort 'missing --rubocop-src PATH' unless options[:rubocop_src]

cop_dept, cop_name = options[:cop].split('/', 2)
abort "expected --cop Dept/Name, got #{options[:cop].inspect}" unless cop_dept && cop_name

rubocop_src = File.expand_path(options[:rubocop_src])
abort "no such directory: #{rubocop_src}" unless Dir.exist?(rubocop_src)

version_file = File.join(rubocop_src, 'lib/rubocop/version.rb')
abort "not a RuboCop checkout (missing #{version_file})" unless File.exist?(version_file)
rubocop_version = File.read(version_file)[/STRING\s*=\s*'([^']+)'/, 1]
abort "could not read RuboCop version from #{version_file}" unless rubocop_version

# --- locate the spec file (and therefore the dept/snake fixture path) ------
#
# RuboCop's own directory layout mirrors the fully qualified class name, but
# rather than reimplementing its CamelCase->snake_case inflection (acronyms
# etc. make that ambiguous), find the spec file whose RSpec.describe matches
# the fully qualified class exactly and derive dept/snake from its path.
describe_pattern = /RSpec\.describe\s+RuboCop::Cop::#{Regexp.escape(cop_dept)}::#{Regexp.escape(cop_name)}\b/
spec_root = File.join(rubocop_src, 'spec/rubocop/cop')
spec_file = Dir.glob(File.join(spec_root, '**/*_spec.rb')).find do |f|
  File.read(f).match?(describe_pattern)
end
abort "no spec file under #{spec_root} describes RuboCop::Cop::#{cop_dept}::#{cop_name}" unless spec_file

dept_snake = spec_file.delete_prefix("#{spec_root}/").delete_suffix('_spec.rb')
out_dir = File.expand_path(File.join(options[:out], dept_snake))
FileUtils.mkdir_p(out_dir)

puts "cop:        Style/#{cop_name}".sub('Style', cop_dept)
puts "rubocop:    #{rubocop_version} (#{rubocop_src})"
puts "spec:       #{spec_file}"
puts "fixtures:   #{out_dir}"

# --- scratch harness ---------------------------------------------------

work = Dir.mktmpdir('port_spec_')
begin
  File.write(File.join(work, 'Gemfile'), <<~GEMFILE)
    source 'https://rubygems.org'
    gem 'rubocop', '#{rubocop_version}'
    gem 'rspec'
  GEMFILE

  lock_out = IO.popen(['bundle', 'lock', '--local'], chdir: work, err: [:child, :out], &:read)
  unless $?.success?
    warn lock_out
    abort "bundle lock --local failed (need rubocop #{rubocop_version} and rspec installed as system gems)"
  end

  captures_path = File.join(work, 'captures.json')
  helper_path = File.join(work, 'capture_helper.rb')
  File.write(helper_path, <<~RUBY)
    ENV['PARSER_ENGINE'] = 'parser_prism'
    require 'rubocop'
    require 'rspec'
    require 'rubocop/rspec/support'
    require 'json'

    CAPTURES = []

    module CaptureOffense
      def current_path
        self.class.parent_groups.reverse.map(&:description) + [RSpec.current_example.description]
      end

      # Only the keys the spec's cop_config actually changes relative to
      # RuboCop's real default configuration for this cop — a spec may set
      # `let(:cop_config)` to a hash that happens to restate a default value
      # (as trailing-comma style specs do for every EnforcedStyleForMultiline
      # variant, including the default `no_comma`), which must not produce a
      # fixture .yml per the contract ("Absent -> RuboCop defaults").
      def cop_config_overrides
        return {} if cop_config.empty?

        real_defaults = RuboCop::ConfigLoader.default_configuration.for_cop(cop_class)
        cop_config.reject { |k, v| real_defaults[k] == v }
      end


      def expect_offense(source, file = nil, severity: nil, chomp: false, **replacements)
        entry = {
          'kind' => 'offense',
          'path' => current_path,
          'file' => file,
          'cop_config' => cop_config_overrides,
          'other_cops' => other_cops,
          'ruby_version' => ruby_version
        }
        result = super
        expected_annotations = parse_annotations(source, **replacements)
        entry['annotated'] = expected_annotations.to_s
        CAPTURES << entry
        @__last_entry = entry
        result
      end

      def expect_correction(correction, loop: true, source: nil)
        result = super
        if @__last_entry
          @__last_entry['correction'] = correction
          @__last_entry['singlepass'] = true unless loop
        end
        result
      end

      def expect_no_offenses(source, file = nil)
        entry = {
          'kind' => 'no_offense',
          'path' => current_path,
          'file' => file,
          'source' => source,
          'cop_config' => cop_config_overrides,
          'other_cops' => other_cops,
          'ruby_version' => ruby_version
        }
        result = super
        CAPTURES << entry
        @__last_entry = entry
        result
      end
    end

    RSpec.configure do |config|
      config.include CaptureOffense
      # Mirror RuboCop's own CI filtering for the Prism engine: examples the
      # upstream suite itself knows are incompatible with Prism parsing are
      # excluded, exactly like elysium's Prism-only engine requires.
      config.filter_run_excluding broken_on: :prism
      config.filter_run_excluding unsupported_on: :prism

      config.before(:each) { @__captures_before = CAPTURES.size }
      config.after(:each) do |example|
        next if example.pending? || example.skipped?
        # expect_offense/expect_no_offenses push their own entry synchronously;
        # examples that never reach that point (a raised error, or direct
        # cop-internals access instead of the expect_offense family) are
        # recorded here as uncaptured for the skip report.
        if CAPTURES.size == @__captures_before
          reason = example.exception ? example.exception.message.lines.first&.chomp : 'did not call expect_offense/expect_no_offenses'
          CAPTURES.push('kind' => 'uncaptured', 'path' => self.class.parent_groups.reverse.map(&:description) + [example.description], 'reason' => reason)
        end
      end
    end

    at_exit do
      File.write(#{captures_path.inspect}, JSON.pretty_generate(CAPTURES))
    end
  RUBY

  spec_dir = File.dirname(spec_file)
  env = { 'BUNDLE_GEMFILE' => File.join(work, 'Gemfile') }
  run_out = IO.popen(
    env, ['bundle', 'exec', 'rspec', '-r', helper_path, File.basename(spec_file)],
    chdir: spec_dir, err: [:child, :out]
  ) { |io| io.read }
  puts run_out.lines.last(15).join

  abort "captures file was not written (rspec crashed before at_exit ran)" unless File.exist?(captures_path)
  captures = JSON.parse(File.read(captures_path))

  # --- fixture generation ----------------------------------------------

  uncaptured = captures.select { |c| c['kind'] == 'uncaptured' }
  captured = captures.reject { |c| c['kind'] == 'uncaptured' }

  min_prism_ruby = 3.3
  default_ruby_version = 3.3 # CopHelper's let(:ruby_version) resolves to this when PARSER_ENGINE=parser_prism (set above)

  skipped = []
  kept = []
  captured.each do |c|
    rv = c['ruby_version']
    if rv && rv.to_f < min_prism_ruby
      skipped << [c['path'], "ruby_version #{rv} is below the Prism parsing minimum (#{min_prism_ruby})"]
    else
      kept << c
    end
  end

  # --- deterministic case naming: snake_case(context segments + it description) ---
  #
  # Keep the description (most specific, rightmost) part intact and drop
  # context tokens from the front only as needed to fit 60 chars; final
  # uniqueness is guaranteed against every name already assigned, not just
  # against same-prefix collisions, so truncation can never silently overwrite
  # an unrelated case.

  def slugify(text)
    text.downcase.gsub(/[^a-z0-9]+/, '_').gsub(/_+/, '_').gsub(/\A_|_\z/, '')
  end

  LEADING_CONNECTORS = /\A(with|when|context|for|behaves like)\s+/i.freeze

  assigned_names = Set.new
  named = kept.map do |c|
    root, *rest, desc = c['path']
    tokens = rest.map { |seg| slugify(seg.sub(LEADING_CONNECTORS, '')) }.reject(&:empty?)
    desc_slug = slugify(desc)
    parts = tokens.dup
    name = (parts + [desc_slug]).join('_')
    while name.length > 60 && !parts.empty?
      parts.shift
      name = (parts + [desc_slug]).join('_')
    end
    name = name[0, 60] if name.length > 60
    name = name.gsub(/_+/, '_').gsub(/\A_|_\z/, '')
    base = name
    i = 1
    while assigned_names.include?(name)
      i += 1
      suffix = "_#{i}"
      name = "#{base[0, 60 - suffix.length]}#{suffix}"
    end
    assigned_names << name
    [name, c]
  end

  named.each do |name, c|
    rb_path = File.join(out_dir, "#{name}.rb")
    content = c['kind'] == 'offense' ? c['annotated'] : c['source']
    File.write(rb_path, content)

    if c['correction']
      File.write(File.join(out_dir, "#{name}.fixed.rb"), c['correction'])
    end
    if c['singlepass']
      File.write(File.join(out_dir, "#{name}.singlepass"), '')
    end
    if c['no_corrections']
      File.write(File.join(out_dir, "#{name}.nofix"), '')
    end

    cop_config = c['cop_config'] || {}
    other_cops = c['other_cops'] || {}
    rv = c['ruby_version']
    needs_ruby_version = rv && rv.to_f != default_ruby_version
    file_comment = c['file']

    next if cop_config.empty? && other_cops.empty? && !needs_ruby_version && !file_comment

    yml_lines = []
    yml_lines << "# file: #{file_comment}" if file_comment
    yml_lines << "AllCops:\n  TargetRubyVersion: #{rv}" if needs_ruby_version
    yml_lines << "#{options[:cop]}:\n#{cop_config.map { |k, v| "  #{k}: #{v.inspect.gsub('"', '')}" }.join("\n")}" unless cop_config.empty?
    other_cops.each do |name_, settings|
      body = settings.is_a?(Hash) ? settings.map { |k, v| "  #{k}: #{v.inspect.gsub('"', '')}" }.join("\n") : "  #{settings}"
      yml_lines << "#{name_}:\n#{body}"
    end
    File.write(File.join(out_dir, "#{name}.yml"), "#{yml_lines.join("\n")}\n")
  end

  puts ''
  puts "ported:     #{named.size} / #{captured.size + uncaptured.size} examples"
  puts "skipped (ruby_version < #{min_prism_ruby}): #{skipped.size}"
  skipped.each { |path, reason| puts "  - #{path.join(' > ')}: #{reason}" }
  puts "not captured (no expect_offense/expect_no_offenses/expect_correction call — likely direct cop-internals access): #{uncaptured.size}"
  uncaptured.each { |c| puts "  - #{c['path'].join(' > ')}#{" (#{c['reason']})" if c['reason']}" }
ensure
  FileUtils.remove_entry(work)
end
