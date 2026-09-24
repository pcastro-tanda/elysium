#!/usr/bin/env ruby
# frozen_string_literal: true

# Development tool only — never a runtime dependency of the linter/rules crates.
#
# Ports a RuboCop cop's RSpec examples into elysium's fixture format (phase3
# contract, section 3): one <case>.rb per expect_offense/expect_no_offenses
# example, <case>.fixed.rb for expect_correction, an empty <case>.nofix for
# expect_no_corrections, an empty <case>.singlepass when the spec used
# expect_correction(..., loop: false), <case>.yml for any cop_config /
# other_cops (including whole-department `Department: {Enabled: false}`
# overrides) / ruby_version override (else RuboCop defaults apply), and
# <case>.offenses when the example built its cop with an explicit `offenses`
# array (one `Cop/Name:line` pair per line) simulating diagnostics from other
# cops that never actually ran -- see `RedundantCopDisableDirective`'s own
# `let(:cop) { described_class.new(config, options, offenses) }`.
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

require 'yaml'
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
      # Enumerates the cop's own supported values for another key (e.g. `SupportedStyles` for
      # `EnforcedStyle`, `SupportedStylesAlignWith` for `EnforcedStyleAlignWith`); metadata about
      # what's *possible*, never a per-case override, so never belongs in a fixture .yml.
      SUPPORTED_STYLES_KEY = /\ASupportedStyles/.freeze
      # Forced by the shared :config context (Enabled => true, AutoCorrect => 'always')
      # regardless of what the spec's own cop_config says; never a real per-case override
      # unless the spec's cop_config itself sets AutoCorrect, which `raw` below already covers.
      SYNTHETIC_KEYS = %w[Enabled AutoCorrect].freeze
      # `AllCops` keys worth reproducing as a peer override when a spec-local `let(:config)`
      # sets them away from RuboCop's real defaults. Excludes every other `AllCops` key (cache
      # paths, formatter selection, doc URLs, …) that never affects cop behavior and would just
      # be noise. `TargetRubyVersion` is handled separately via `ruby_version`/`needs_ruby_version`.
      ALL_COPS_OVERRIDE_KEYS = %w[
        TargetRubyVersion TargetRailsVersion StringLiteralsFrozenByDefault
        ActiveSupportExtensionsEnabled DisabledByDefault EnabledByDefault NewCops
      ].freeze

      def current_path
        self.class.parent_groups.reverse.map(&:description) + [RSpec.current_example.description]
      end

      # A spec may set an option to nil that RuboCop defaults to a list or a
      # boolean. Ruby treats nil as falsy, so a nil boolean equals false; a nil
      # list is a real override (RuboCop code paths distinguish `nil` from
      # `[]`, e.g. `URISchemes: nil` matches every scheme) and is kept as `~`.
      def normalize_option(value, default_value)
        return value unless value.nil?

        case default_value
        when true, false then false
        else value
        end
      end

      def diff_against_defaults(hash, real_defaults, exclude: [], keys: hash.keys)
        keys.each_with_object({}) do |k, acc|
          next if exclude.include?(k) || k.match?(SUPPORTED_STYLES_KEY)

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

      # Peer cops, AllCops, and whole-department keys the spec's *effective* config (cop.config,
      # the merged RuboCop::Config actually handed to the cop under test) sets away from
      # RuboCop's real defaults. Most specs only ever touch peer cops via `let(:other_cops)`,
      # which the shared :config context merges into `config` alongside the cop under test --
      # already captured verbatim as `other_cops` at entry-creation time. A few specs (e.g.
      # Layout::IndentationWidth, Layout::LineLength) instead define their own `let(:config) {
      # RuboCop::Config.new(...) }` naming peer cops directly, bypassing `other_cops` entirely;
      # walking every `cop.config` key that looks like a cop name (contains '/') and diffing its
      # `for_cop` value against real defaults catches those too. Unlike the cop-under-test diff,
      # SYNTHETIC_KEYS is NOT excluded here: `Enabled`/`AutoCorrect` on a *peer* cop are genuine,
      # meaningful overrides (e.g. `Layout/LineLength: { Enabled: false }`), never a test-harness
      # artifact. A `nil` peer value, unlike a `nil` cop_config value, is never kept as `~`: it
      # only means the manually-built `RuboCop::Config` never mentioned that peer cop's key at
      # all (the peer was never configured for *any* option, cop_config or otherwise), so it's
      # unset noise, not an intentional override. `AllCops` is restricted to a small allowlist of
      # keys that actually affect cop behavior (formatter/cache/doc-URL keys are noise);
      # `TargetRubyVersion` is excluded here -- already handled separately via `ruby_version`.
      # A bare CamelCase top-level key with no '/' (e.g. `Metrics`) is a whole-department
      # override -- RuboCop's real default config never has one (only `AllCops` is bare), so any
      # other bare CamelCase key present is necessarily a spec-local `RuboCop::Config.new('Metrics'
      # => { 'Enabled' => false })`-style override, kept verbatim (department hashes are small and
      # never contain doc-only keys worth filtering).
      def effective_peer_overrides
        default_config = RuboCop::ConfigLoader.default_configuration
        effective = cop.config
        peers = {}
        effective.to_h.each_key do |key|
          if key != 'AllCops' && !key.include?('/') && key =~ /\\A[A-Z]/
            peers[key] = effective[key]
            next
          end
          next unless key.include?('/')
          next if key == cop_class.cop_name

          cop_effective = effective.for_cop(key)
          real_defaults = default_config.for_cop(key)
          diffed = diff_against_defaults(
            cop_effective, real_defaults,
            exclude: DOC_ONLY_KEYS,
            keys: cop_effective.keys | real_defaults.keys
          )
          diffed.reject! { |_, v| v.nil? }
          peers[key] = diffed unless diffed.empty?
        end
        all_cops_effective = effective['AllCops'] || {}
        all_cops_defaults = default_config['AllCops'] || {}
        all_cops_diff = diff_against_defaults(
          all_cops_effective, all_cops_defaults,
          exclude: DOC_ONLY_KEYS + ['TargetRubyVersion'],
          keys: ALL_COPS_OVERRIDE_KEYS
        )
        all_cops_diff.reject! { |_, v| v.nil? }
        peers['AllCops'] = all_cops_diff unless all_cops_diff.empty?
        peers
      end

      # The example's injected `offenses` array, when its example group defines one (only
      # `RedundantCopDisableDirective`'s spec does, via its own `let(:cop) { described_class.new(
      # config, options, offenses) }` overriding the shared :config context's normal two-arg
      # `cop`), as plain `{ 'cop' => ..., 'line' => ... }` hashes -- never a real diagnostic any
      # rule produced, so the fixture harness must feed it to `file_finish` synthetically rather
      # than expect it to appear from an actual lint pass.
      def injected_offenses
        return [] unless respond_to?(:offenses)

        value = offenses
        return [] unless value.is_a?(Array) && value.all? { |o| o.is_a?(::RuboCop::Cop::Offense) }

        value.map { |o| { 'cop' => o.cop_name, 'line' => o.line } }
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
        entry['other_cops'] = effective_peer_overrides.merge(entry['other_cops'])
        entry['offenses'] = injected_offenses
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
          lines = render_source.each_line.to_a
          # An offense past the last line (RuboCop reports a missing final blank line at
          # the position after the trailing "\\n") needs a physical line to hang off: the
          # harness splits on "\\n" and so already sees that empty last line.
          phantom = offenses.any? { |o| o.line > lines.length }
          lines << "\\n" if phantom
          annotated = ::RuboCop::RSpec::ExpectOffense::AnnotatedSource
                        .new(lines, [])
                        .with_offense_annotations(offenses)
                        .to_s
          # Without the padding newline the harness rebuilds exactly `plain`: for the
          # phantom line, ["x = 0", ""] joined by "\\n" is "x = 0\\n".
          annotated = annotated.delete_suffix("\\n") if phantom || !plain.end_with?("\\n")
          entry = {
            'kind' => 'offense',
            'path' => current_path,
            'file' => nil,
            'cop_config' => raw.merge(effective_cop_config_extra(raw)),
            'other_cops' => effective_peer_overrides.merge(other_cops),
            'offenses' => injected_offenses,
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
        entry['other_cops'] = effective_peer_overrides.merge(entry['other_cops'])
        entry['offenses'] = injected_offenses
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

  # Serializes one option value as an inline YAML scalar or flow sequence,
  # quoting through Psych so regex sources and special strings round-trip.
  def yaml_value(v)
    case v
    when nil then '~'
    when Array then "[#{v.map { |e| yaml_value(e) }.join(', ')}]"
    when Hash then "{#{v.map { |k, e| "#{yaml_value(k.to_s)}: #{yaml_value(e)}" }.join(', ')}}"
    when String then YAML.dump(v).sub(/\A---\s*/, '').chomp
    else v.to_s
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
    if c['offenses'] && !c['offenses'].empty?
      lines = c['offenses'].map { |o| "#{o['cop']}:#{o['line']}" }
      File.write(File.join(out_dir, "#{name}.offenses"), "#{lines.join("\n")}\n")
    end

    cop_config = c['cop_config'] || {}
    other_cops = (c['other_cops'] || {}).dup
    all_cops = other_cops.delete('AllCops') || {}
    rv = c['ruby_version']
    needs_ruby_version = rv && rv.to_f != default_ruby_version
    all_cops = { 'TargetRubyVersion' => rv }.merge(all_cops) if needs_ruby_version
    file_comment = c['file']

    next if cop_config.empty? && other_cops.empty? && all_cops.empty? && !file_comment

    yml_lines = []
    yml_lines << "# file: #{file_comment}" if file_comment
    yml_lines << "AllCops:\n#{all_cops.map { |k, v| "  #{k}: #{yaml_value(v)}" }.join("\n")}" unless all_cops.empty?
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
