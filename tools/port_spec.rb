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
    Dir[File.join(#{rubocop_src.inspect}, 'spec/support/**/*.rb')].sort.each { |f| require f }
    require 'json'

    CAPTURES = []

    module CaptureOffense
      # Pure documentation/metadata keys never belong in a fixture .yml override.
      DOC_ONLY_KEYS = %w[
        Description StyleGuide StyleGuideAlias Reference References
        VersionAdded VersionChanged VersionRemoved Details DocumentationReference
      ].freeze
      # Forced by the shared :config context (Enabled => true, AutoCorrect => 'always')
      # regardless of what the spec's own cop_config says; never a real per-case override
      # unless the spec's cop_config itself sets AutoCorrect, which \`raw\` below already covers.
      SYNTHETIC_KEYS = %w[Enabled AutoCorrect].freeze

      def current_path
        self.class.parent_groups.reverse.map(&:description) + [RSpec.current_example.description]
      end

      # `nil` in a spec's cop_config (e.g. `'AllowedPatterns' => nil`) means "unset", which is
      # only meaningfully comparable to the real default once normalized to that option's shape
      # (boolean options: false; list options: []) instead of literal nil.
      def normalize_option(value, default_value)
        return value unless value.nil?

        case default_value
        when true, false then false
        when Array then []
        else value
        end
      end

      def diff_against_defaults(hash, real_defaults, exclude: [], keys: hash.keys)
        keys.each_with_object({}) do |k, acc|
          next if exclude.include?(k)

          normalized = normalize_option(hash[k], real_defaults[k])
          acc[k] = normalized unless normalized == real_defaults[k]
        end
      end

      # Only the keys the spec's *effective* config actually changes relative to RuboCop's real
      # default configuration for this cop. Most specs build their config via the shared :config
      # context (defaults.merge(cop_config)), so diffing the raw `cop_config` let value already
      # finds every override. A few specs (e.g. Layout::LineLength) define their own `let(:config)`
      # that bypasses that merge entirely, so an option a spec never mentions can still run with a
      # non-default effective value; diffing the cop's actual merged config against real defaults
      # (skipping doc-only and test-harness-synthetic keys already covered by `raw`) catches those.
      def cop_config_overrides
        real_defaults = RuboCop::ConfigLoader.default_configuration.for_cop(cop_class)
        cop_config.empty? ? {} : diff_against_defaults(cop_config, real_defaults)
      end

      # Supplemental diff against the cop's actual merged config, computed only after the real
      # expect_offense/expect_no_offenses has run so building/reading `cop` here never disturbs
      # state (e.g. exclude_limit's config_to_allow_offenses tracking) that the investigation
      # itself depends on. Walks the union of both key sets (not just `effective`'s own keys) so
      # a key that's simply absent from `effective` — because a spec-local `let(:config)` skipped
      # the normal defaults merge — but has a non-empty/non-false real default is still caught.
      # Deliberately does NOT exclude keys already in `raw`: a spec-local `let(:config)` that
      # ignores `cop_config` entirely (as Layout::LineLength's own top-of-file override does)
      # makes `raw` (built from the `cop_config` let) stale for every key, so the effective,
      # actually-applied value must win on conflict — callers merge as `raw.merge(extra)`.
      def effective_cop_config_extra(raw)
        real_defaults = RuboCop::ConfigLoader.default_configuration.for_cop(cop_class)
        effective = cop.config.for_cop(cop_class)
        diff_against_defaults(
          effective, real_defaults,
          exclude: DOC_ONLY_KEYS + SYNTHETIC_KEYS,
          keys: effective.keys | real_defaults.keys
        )
      end

      def expect_offense(source, file = nil, severity: nil, chomp: false, **replacements)
        raw = cop_config_overrides
        entry = {
          'kind' => 'offense',
          'path' => current_path,
          'file' => file,
          'cop_config' => raw,
          'other_cops' => other_cops,
          'ruby_version' => ruby_version
        }
        result = super
        entry['cop_config'] = raw.merge(effective_cop_config_extra(raw))
        # Reuse the offenses `super` already found instead of re-parsing annotations via
        # `parse_annotations`, which also calls `set_formatter_options` and would wipe out
        # state (e.g. exclude_limit's config_to_allow_offenses tracking) that `super`'s own
        # investigation just populated.
        parsed = ::RuboCop::RSpec::ExpectOffense::AnnotatedSource.parse(format_offense(source, **replacements))
        annotated = parsed.with_offense_annotations(result).to_s
        # `chomp: true` means RuboCop itself linted `plain_source.chomp` (one trailing "\\n"
        # stripped from the code, before annotations existed). The fixture harness reconstructs
        # source by splitting the .rb on "\\n", dropping annotation lines, and rejoining with
        # "\\n", so the file's own trailing newline becomes the source's; stripping exactly one
        # trailing "\\n" from the annotated dump reproduces that chomp at the file level.
        annotated = annotated.delete_suffix("\\n") if chomp
        entry['annotated'] = annotated
        CAPTURES << entry
        @__last_entry = entry
        result
      end

      def expect_correction(correction, loop: true, source: nil)
        if source && !@__last_entry
          # No preceding expect_offense in this example: run the cop ourselves (the same
          # investigation path RuboCop's own expect_correction takes) so this still yields a
          # real annotated .rb instead of falling through as a bare, unannotated source (which
          # the harness would treat as an expect_no_offenses case and fail).
          raw = cop_config_overrides
          expected_annotations = parse_annotations(source, raise_error: false)
          plain = expected_annotations.plain_source
          @processed_source = parse_processed_source(plain)
          offenses = _investigate(cop, @processed_source)
          # `with_offense_annotations` concatenates an inserted annotation directly onto the
          # preceding line when that line lacks its own trailing "\\n" (plain `plain`-less
          # source, e.g. `source: 'x = 0'`). Pad with one synthetic "\\n" so the annotation
          # always lands on its own physical line, then strip that padding back off the
          # rendered result — same trick as `chomp` above — so the .rb file's own
          # trailing-newline status still matches `plain`, not the padded copy.
          render_source = plain.end_with?("\\n") ? plain : "\#{plain}\\n"
          annotated = ::RuboCop::RSpec::ExpectOffense::AnnotatedSource
                        .new(render_source.each_line.to_a, [])
                        .with_offense_annotations(offenses)
                        .to_s
          annotated = annotated.delete_suffix("\\n") unless plain.end_with?("\\n")
          entry = {
            'kind' => 'offense',
            'path' => current_path,
            'file' => nil,
            'cop_config' => raw.merge(effective_cop_config_extra(raw)),
            'other_cops' => other_cops,
            'ruby_version' => ruby_version,
            'annotated' => annotated
          }
          CAPTURES << entry
          @__last_entry = entry
        end
        result = super
        if @__last_entry
          @__last_entry['correction'] = correction
          @__last_entry['singlepass'] = true unless loop
        end
        result
      end

      def expect_no_corrections
        result = super
        @__last_entry['no_corrections'] = true if @__last_entry
        result
      end

      def expect_no_offenses(source, file = nil)
        raw = cop_config_overrides
        entry = {
          'kind' => 'no_offense',
          'path' => current_path,
          'file' => file,
          'source' => source,
          'cop_config' => raw,
          'other_cops' => other_cops,
          'ruby_version' => ruby_version
        }
        result = super
        entry['cop_config'] = raw.merge(effective_cop_config_extra(raw))
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

  def yaml_value(v)
    case v
    when nil then '~'
    when Array then "[#{v.map(&:to_s).join(', ')}]"
    else v.inspect.gsub('"', '')
    end
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
    yml_lines << "#{options[:cop]}:\n#{cop_config.map { |k, v| "  #{k}: #{yaml_value(v)}" }.join("\n")}" unless cop_config.empty?
    other_cops.each do |name_, settings|
      body = settings.is_a?(Hash) ? settings.map { |k, v| "  #{k}: #{yaml_value(v)}" }.join("\n") : "  #{yaml_value(settings)}"
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
