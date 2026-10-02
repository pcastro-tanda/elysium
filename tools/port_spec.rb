#!/usr/bin/env ruby
# frozen_string_literal: true

# Development tool only — never a runtime dependency of the linter/rules crates.
#
# Ports a RuboCop cop's examples into elysium's fixture format (phase3
# contract, section 3): one <case>.rb per expect_offense/expect_no_offenses
# example, <case>.fixed.rb for expect_correction, an empty <case>.nofix for
# expect_no_corrections, an empty <case>.singlepass when the spec used
# expect_correction(..., loop: false), <case>.yml for any cop_config /
# other_cops (including whole-department `Department: {Enabled: false}`
# overrides) / ruby_version override (else RuboCop defaults apply), and
# <case>.offenses when the example built its cop with an explicit `offenses`
# array (one `Cop/Name:line` pair per line) simulating diagnostics from other
# cops that never actually ran -- see `RedundantCopDisableDirective`'s own
# `let(:cop) { described_class.new(config, options, offenses) }`. An extension
# spec's stubbed `gem_versions` (what `requires_gem` checks) is recorded in the
# yml as a `# gem_versions: rack=3.1.0, other=1.2` comment line.
#
# It works by actually running the real RuboCop spec against the real cop,
# with expect_offense/expect_correction/expect_no_offenses/expect_no_corrections
# monkeypatched to record their exact (already-interpolated) arguments before
# delegating to RuboCop's own implementation. Every ported fixture is therefore
# taken verbatim from a passing assertion against the real cop — not a manual
# transcription.
#
# Cops of the extension gems in tools/extension_gems.rb (Rails/, Performance/,
# ThreadSafety/, Minitest/, Sorbet/) are ported the same way from that gem's
# own suite, loaded through its own spec_helper/test_helper: RSpec for
# rubocop-rails, rubocop-performance and rubocop-thread_safety; Minitest for
# rubocop-minitest and rubocop-sorbet, whose rubocop-minitest
# assert_offense/assert_no_offenses/assert_correction/assert_no_corrections are
# recorded and delegated the same way.
#
# Usage:
#   ruby tools/port_spec.rb --cop Style/TrailingCommaInArguments \
#     --rubocop-src /Users/paulo/Work/lab/corpus/rubocop-1.91.0 \
#     --out crates/rules/fixtures
#   ruby tools/port_spec.rb --cop Rails/ApplicationRecord \
#     --rubocop-src /Users/paulo/Work/lab/corpus/rubocop-1.91.0 \
#     [--gem-src rails=/Users/paulo/Work/lab/corpus/rubocop-rails-2.38.0]
#
# --gem-src KEY=PATH (comma-separated or repeated; KEY is the gem's key in
# tools/extension_gems.rb) names an extension gem's source checkout; without
# it the `<gem>-<version>` directory next to --rubocop-src is used. Its
# version (read from the gem's own version file) must be the one
# tools/extension_gems.rb pins.
#
# Requires: the exact RuboCop version pinned by --rubocop-src's
# lib/rubocop/version.rb (and, for an extension cop, the pinned gem version
# plus the suite dependencies tools/extension_gems.rb lists), and `rspec`,
# already installed as system gems (checked with `gem list`). No network
# access and no modification of --rubocop-src or --gem-src are performed; a
# scratch Gemfile is written under a temp directory and resolved with
# `bundle lock --local`.

require 'yaml'
require 'optparse'
require 'fileutils'
require 'tmpdir'
require 'json'
require 'set'
require_relative 'extension_gems'

options = { out: 'crates/rules/fixtures', gem_src: {} }
OptionParser.new do |opts|
  opts.banner = 'Usage: port_spec.rb --cop Dept/Name --rubocop-src PATH [--gem-src KEY=PATH,...] [--out DIR]'
  opts.on('--cop COP', 'Cop name, e.g. Style/TrailingCommaInArguments') { |v| options[:cop] = v }
  opts.on('--rubocop-src PATH', 'Path to a full RuboCop source checkout (has lib/ and spec/)') { |v| options[:rubocop_src] = v }
  opts.on('--gem-src KEY=PATH', 'Extension gem source checkout(s), e.g. rails=PATH,minitest=PATH (default: <gem>-<version> next to --rubocop-src)') do |v|
    options[:gem_src].merge!(ExtensionGems.parse_sources(v))
  end
  opts.on('--out DIR', 'Fixture root (default crates/rules/fixtures)') { |v| options[:out] = v }
  opts.on('--spec FILE', 'Port only this spec file (default: every spec describing the cop)') { |v| options[:spec] = v }
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

# An extension cop's examples come from its gem's own suite, at the pinned version.
extension = ExtensionGems.for_department(cop_dept)
if extension
  gem_src = extension.source(options[:gem_src], rubocop_src)
  unless gem_src && Dir.exist?(gem_src)
    abort "no #{extension.gem} source: pass --gem-src #{extension.key}=PATH (a checkout of #{extension.repository} at #{extension.tag})"
  end
  gem_version = extension.version_of(gem_src)
  abort "could not read the #{extension.gem} version from #{File.join(gem_src, extension.version_file)}" unless gem_version
  unless gem_version == extension.version
    abort "#{gem_src} is #{extension.gem} #{gem_version}; tools/extension_gems.rb pins #{extension.version}"
  end
end
framework = extension ? extension.framework : :rspec

# --- locate the spec file (and therefore the dept/snake fixture path) ------
#
# RuboCop's own directory layout mirrors the fully qualified class name, but
# rather than reimplementing its CamelCase->snake_case inflection (acronyms
# etc. make that ambiguous), find the spec file whose RSpec.describe matches
# the fully qualified class exactly and derive dept/snake from its path.
# Anchored at line start: other specs (InternalAffairs) quote describes inside heredocs.
# An extension cop's fixture department is its department's directory under
# the gem's lib/rubocop/cop (rubocop-sorbet nests some cops a level deeper,
# `sorbet/sigils/`, still in `RuboCop::Cop::Sorbet`); its cop source is the
# file there defining the class, and a Minitest suite's test is
# `<cop source stem>_test.rb`.
describe_pattern = /^RSpec\.describe[\s(]+RuboCop::Cop::#{Regexp.escape(cop_dept)}::#{Regexp.escape(cop_name)}\b/
spec_root = File.join(extension ? gem_src : rubocop_src, extension ? extension.spec_root : 'spec/rubocop/cop')
lib_root = File.join(extension ? gem_src : rubocop_src, 'lib/rubocop/cop')
class_pattern = /^\s*class #{Regexp.escape(cop_name)} < /
if extension
  fixture_dept = ExtensionGems.snake(cop_dept)
  cop_source = Dir.glob(File.join(lib_root, fixture_dept, '**/*.rb')).sort.find { |f| File.read(f).match?(class_pattern) }
  abort "no cop source under #{lib_root}/#{fixture_dept} defines #{cop_name}" unless cop_source
end
spec_files =
  if framework == :minitest
    Dir.glob(File.join(spec_root, '**', "#{File.basename(cop_source, '.rb')}_test.rb")).sort
  else
    Dir.glob(File.join(spec_root, '**/*_spec.rb')).select do |f|
      File.read(f).match?(describe_pattern)
    end.sort
  end
abort "no spec file under #{spec_root} describes RuboCop::Cop::#{cop_dept}::#{cop_name}" if spec_files.empty?

# Some cops split their spec across files (Style/ConditionalAssignment has
# `_assign_in_condition_spec.rb` and `_assign_to_condition_spec.rb`): port each
# into the cop's one fixture dir, case names prefixed by the file's extra stem.
if spec_files.size > 1 && !options[:spec]
  gem_src_args = extension ? ['--gem-src', "#{extension.key}=#{gem_src}"] : []
  spec_files.each do |f|
    system(RbConfig.ruby, __FILE__, '--cop', options[:cop], '--rubocop-src', rubocop_src, *gem_src_args,
           '--out', options[:out], '--spec', f) || abort("porting #{f} failed")
  end
  exit
end
spec_file = options[:spec] ? File.expand_path(options[:spec]) : spec_files.first

unless extension
  spec_dept = File.dirname(spec_file.delete_prefix("#{spec_root}/"))
  cop_source = Dir.glob(File.join(rubocop_src, 'lib/rubocop/cop', spec_dept, '*.rb')).find do |f|
    File.read(f).match?(/^\s*class #{Regexp.escape(cop_name)} < /)
  end
  abort "no cop source under lib/rubocop/cop/#{spec_dept} defines #{cop_name}" unless cop_source
  fixture_dept = spec_dept
end
cop_stem = File.basename(cop_source, '.rb')
spec_stem = File.basename(spec_file, '.rb').delete_suffix(framework == :minitest ? '_test' : '_spec')
case_prefix = spec_stem == cop_stem ? '' : spec_stem.delete_prefix(cop_stem).delete_prefix('_')
dept_snake = File.join(fixture_dept, cop_stem)
out_dir = File.expand_path(File.join(options[:out], dept_snake))
FileUtils.mkdir_p(out_dir)

puts "cop:        Style/#{cop_name}".sub('Style', cop_dept)
puts "rubocop:    #{rubocop_version} (#{rubocop_src})"
puts "gem:        #{extension.gem} #{gem_version} (#{gem_src})" if extension
puts "spec:       #{spec_file}"
puts "fixtures:   #{out_dir}"

# --- scratch harness ---------------------------------------------------

work = Dir.mktmpdir('port_spec_')
begin
  # RuboCop >= 1.91's spec/support loads its MCP server, which needs the
  # `mcp` gem; harmless for older sources. An extension suite loads only its
  # own spec_helper/test_helper, so it needs its own dependencies instead.
  gemfile =
    if extension
      <<~GEMFILE
        source 'https://rubygems.org'
        gem 'rubocop', '#{rubocop_version}'
        gem '#{extension.gem}', '#{extension.version}'
        #{extension.gemfile.join("\n")}
      GEMFILE
    else
      <<~GEMFILE
        source 'https://rubygems.org'
        gem 'rubocop', '#{rubocop_version}'
        gem 'rspec'
        gem 'mcp', '~> 0.6'
      GEMFILE
    end
  File.write(File.join(work, 'Gemfile'), gemfile)

  lock_out = IO.popen(['bundle', 'lock', '--local'], chdir: work, err: [:child, :out], &:read)
  unless $?.success?
    warn lock_out
    requirement = extension ? "rubocop #{rubocop_version}, #{extension.gem} #{extension.version} and the suite's gems (tools/extension_gems.rb)" : "rubocop #{rubocop_version} and rspec"
    abort "bundle lock --local failed (need #{requirement} installed as system gems)"
  end

  captures_path = File.join(work, 'captures.json')
  helper_path = File.join(work, 'capture_helper.rb')
  helper = +<<~RUBY
    ENV['PARSER_ENGINE'] = 'parser_prism'
  RUBY
  if extension
    # The suite's own spec_helper/test_helper: it loads the gem (from
    # --gem-src's lib/, first on the load path) and its test support, and its
    # cop helper integrates the gem's default configuration as a plugin.
    helper << <<~RUBY
      require #{File.join(gem_src, extension.support).inspect}
    RUBY
  else
    helper << <<~RUBY
      require 'rubocop'
      require 'rspec'
      require 'rubocop/rspec/support'
      Dir[File.join(#{rubocop_src.inspect}, 'spec/support/**/*.rb')].sort.each { |f| require f }
      Dir[File.join(#{rubocop_src.inspect}, 'spec/core_ext/**/*.rb')].sort.each { |f| require f }
    RUBY
  end
  helper << <<~RUBY
    require 'json'

    CAPTURES = []
    # Every (peer cop, option key) the code under test reads from a `Config#for_cop` hash
    # during an example. A key the spec's bespoke `RuboCop::Config` never sets reads as nil
    # there (a peer it never mentions is `{}` plus `Enabled`), whereas the fixture harness
    # merges RuboCop's real defaults; `effective_peer_overrides` reproduces the spec's view
    # for exactly these keys. Paused while the capture code itself inspects the config.
    PEER_READS = Hash.new { |h, name| h[name] = Set.new }
    module PeerRecording
      class << self
        attr_accessor :paused
      end
    end
    module RecordKeyReads
      %i[[] fetch key? dig].each do |meth|
        define_method(meth) do |key, *rest, &blk|
          PEER_READS[@__port_peer_name] << key unless PeerRecording.paused
          super(key, *rest, &blk)
        end
      end
    end
    module RecordPeerReads
      def for_cop(cop)
        conf = super
        unless conf.frozen? || conf.is_a?(RecordKeyReads)
          conf.instance_variable_set(:@__port_peer_name, cop.respond_to?(:cop_name) ? cop.cop_name : cop.to_s)
          conf.extend(RecordKeyReads)
        end
        conf
      end

      # `AllCops` keys are recorded under 'AllCops', so `effective_peer_overrides` states
      # exactly the ones the spec's config sets or the code under test reads: a suite whose
      # bespoke config omits a key the merged defaults set (rubocop-rails' own
      # `ActiveSupportExtensionsEnabled: true`) only differs where the difference is seen.
      def for_all_cops
        conf = super
        unless conf.frozen? || conf.is_a?(RecordKeyReads)
          conf.instance_variable_set(:@__port_peer_name, 'AllCops')
          conf.extend(RecordKeyReads)
        end
        conf
      end
    end

    # What every capture records about the cop under test's configuration, whatever the
    # suite's assertion helpers (`CaptureOffense` for RSpec, `CaptureAssertion` for Minitest):
    # each includer defines `port_cop_class`, `port_cop_config` and `port_other_cops`.
    module CaptureConfig
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

      # `literal` keys skip `normalize_option`: a peer key the cop under test read may be
      # compared with `== false`, where nil and false differ, so it keeps its exact value.
      def diff_against_defaults(hash, real_defaults, exclude: [], keys: hash.keys, literal: [])
        keys.each_with_object({}) do |k, acc|
          next if exclude.include?(k) || k.match?(SUPPORTED_STYLES_KEY)

          value = literal.include?(k) ? hash[k] : normalize_option(hash[k], real_defaults[k])
          acc[k] = value unless value == real_defaults[k]
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
        real_defaults = RuboCop::ConfigLoader.default_configuration.for_cop(port_cop_class)
        port_cop_config.empty? ? {} : diff_against_defaults(port_cop_config, real_defaults)
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
        real_defaults = RuboCop::ConfigLoader.default_configuration.for_cop(port_cop_class)
        effective = cop.config.for_cop(port_cop_class)
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
      # never contain doc-only keys worth filtering). An extension's defaults do have one
      # (`Sorbet:`), which a suite building its cop from those defaults leaves unchanged.
      # A key the cop under test actually read (`PEER_READS`) keeps a nil value as `~`, and a
      # peer the spec's config never mentions is emitted for exactly its read keys: there,
      # `for_cop` gave `{}` plus `Enabled`, so e.g. `conf['Enabled'] &&
      # conf['EnforcedStyleAlignWith']` is nil, not the harness's merged default.
      def effective_peer_overrides
        PeerRecording.paused = true
        default_config = RuboCop::ConfigLoader.default_configuration
        effective = cop.config
        configured = effective.to_h.keys
        peers = {}
        (configured | PEER_READS.keys.select { |name| name.include?('/') }).each do |key|
          if key != 'AllCops' && !key.include?('/') && key =~ /\\A[A-Z]/
            peers[key] = effective[key] unless effective[key] == default_config[key]
            next
          end
          next unless key.include?('/')
          next if key == port_cop_class.cop_name

          read_keys = PEER_READS.fetch(key, Set.new)
          cop_effective = effective.for_cop(key)
          real_defaults = default_config.for_cop(key)
          diffed = diff_against_defaults(
            cop_effective, real_defaults,
            exclude: DOC_ONLY_KEYS,
            keys: configured.include?(key) ? cop_effective.keys | real_defaults.keys : read_keys.to_a,
            literal: read_keys
          )
          diffed.reject! { |k, v| v.nil? && !read_keys.include?(k) }
          peers[key] = diffed unless diffed.empty?
        end
        all_cops_effective = effective['AllCops'] || {}
        all_cops_defaults = default_config['AllCops'] || {}
        all_cops_read = PEER_READS.fetch('AllCops', Set.new)
        all_cops_diff = diff_against_defaults(
          all_cops_effective, all_cops_defaults,
          exclude: DOC_ONLY_KEYS + ['TargetRubyVersion'],
          keys: ALL_COPS_OVERRIDE_KEYS.select { |k| all_cops_effective.key?(k) || all_cops_read.include?(k) }
        )
        all_cops_diff.reject! { |_, v| v.nil? }
        peers['AllCops'] = all_cops_diff unless all_cops_diff.empty?
        peers
      ensure
        PeerRecording.paused = false
      end

      # `other_cops` is the spec's literal input; it wins key by key over the effective view.
      def merge_peers(effective, literal)
        effective.merge(literal) { |_, a, b| a.is_a?(Hash) && b.is_a?(Hash) ? a.merge(b) : b }
      end

      # The gem versions an extension spec stubs into `Config#gem_versions_in_target`
      # (its `let(:gem_versions)`, e.g. `{ 'rack' => '3.1.0' }`), as strings. `railties` is
      # left out: the Rails suite's support code derives it from `rails_version`, which the
      # case's `AllCops: TargetRailsVersion` already carries. Core RuboCop cases never record it.
      def port_gem_versions
        return {} unless #{extension ? 'true' : 'false'} && respond_to?(:gem_versions, true)

        gem_versions.to_h { |name, version| [name.to_s, version.to_s] }.reject { |name, _| name == 'railties' }
      end

      # A spec may pass a `Tempfile` as `file` (`Lint/ScriptPermission`);
      # upstream then lints the source under that file's random temp path, so
      # record a stable stand-in rather than the object's `inspect`.
      def fixture_file(file)
        file.respond_to?(:path) ? 'tempfile' : file
      end

      # Whether RuboCop prefixes the cop under test's messages with its name
      # (`MessageAnnotator#display_cop_names?`). A bespoke spec config never sets
      # `AllCops/DisplayCopNames`; a suite building the cop straight from the merged defaults
      # (rubocop-sorbet's `@cop = Foo.new`) gets their `DisplayCopNames: true`.
      def port_display_cop_names?
        PeerRecording.paused = true
        annotator = RuboCop::Cop::MessageAnnotator.new(
          cop.config, cop.cop_name, cop.cop_config, cop.instance_variable_get(:@options) || {}
        )
        annotator.send(:display_cop_names?) ? true : false
      ensure
        PeerRecording.paused = false
      end
    end

    def write_captures
      # Examples whose source holds non-UTF-8 bytes (e.g. `"\\xC0"` in a heredoc) can't be
      # serialized, and the fixture harness only reads UTF-8 sources anyway.
      captures = CAPTURES.map do |entry|
        JSON.generate(entry)
        entry
      rescue JSON::GeneratorError, Encoding::UndefinedConversionError
        { 'kind' => 'uncaptured', 'path' => entry['path'], 'reason' => 'source is not valid UTF-8' }
      end
      File.write(#{captures_path.inspect}, JSON.pretty_generate(captures))
    end

  RUBY
  if framework == :minitest
    helper << <<~RUBY
      # Minitest suites (rubocop-minitest, rubocop-sorbet): rubocop-minitest's `AssertOffense`
      # helpers, which `Minitest::Test` includes. The cop under test is whichever one the test
      # built (`cop`, possibly the test's own `@cop`), so its configuration is `cop.config`.
      # Written against both helper versions in use: 0.40 (rubocop-minitest's own suite) has
      # `cop_config`/`other_cops` and a `RuboCop::Config` built from them; 0.38.1 (what
      # rubocop-sorbet's lockfile pins) has neither, and its suite builds every cop itself.
      module CaptureAssertion
        include CaptureConfig

        def port_path
          [self.class.name, name.delete_prefix('test_')]
        end

        # Minitest shuffles tests; captures are written in test definition order, then the
        # assertion's order within its test, so case naming does not depend on the seed.
        def port_order
          file, line = self.class.instance_method(name).source_location
          @__capture_seq = (@__capture_seq || 0) + 1
          [file.to_s, line.to_i, @__capture_seq]
        end

        def port_cop_class
          cop.class
        end

        # rubocop-minitest's own `cop_config` is the hash a test assigns. A suite helper may
        # redefine it -- rubocop-sorbet's builds a whole `RuboCop::Config` from an argument --
        # and then `cop.config`'s effective diff carries every override.
        def port_cop_config
          return {} unless respond_to?(:cop_config, true) && method(:cop_config).arity.zero?

          value = cop_config
          value.is_a?(Hash) ? value : {}
        end

        def port_other_cops
          value = respond_to?(:other_cops, true) ? other_cops : {}
          value.is_a?(Hash) ? value : {}
        end

        def assert_offense(source, file = nil, **replacements)
          raw = cop_config_overrides
          entry = {
            'kind' => 'offense',
            'path' => port_path,
            'order' => port_order,
            'file' => fixture_file(file),
            'cop_config' => raw,
            'other_cops' => port_other_cops,
            'ruby_version' => ruby_version
          }
          result = super
          entry['cop_config'] = raw.merge(effective_cop_config_extra(raw))
          entry['other_cops'] = merge_peers(effective_peer_overrides, entry['other_cops'])
          entry['display_cop_names'] = true if port_display_cop_names?
          # `super` passed, so its offenses render exactly the expected annotations.
          parsed = ::RuboCop::RSpec::ExpectOffense::AnnotatedSource.parse(format_offense(source, **replacements))
          entry['annotated'] = parsed.with_offense_annotations(@offenses).to_s
          CAPTURES << entry
          @__last_entry = entry
          result
        end

        def assert_correction(correction, loop: true)
          result = super
          if @__last_entry
            @__last_entry['correction'] = correction
            @__last_entry['singlepass'] = true unless loop
          end
          result
        end

        def assert_no_corrections
          result = super
          @__last_entry['no_corrections'] = true if @__last_entry
          result
        end

        def assert_no_offenses(source, file = nil)
          raw = cop_config_overrides
          entry = {
            'kind' => 'no_offense',
            'path' => port_path,
            'order' => port_order,
            'file' => fixture_file(file),
            'source' => source,
            'cop_config' => raw,
            'other_cops' => port_other_cops,
            'ruby_version' => ruby_version,
            'schema' => port_schema
          }
          result = super
          entry['cop_config'] = raw.merge(effective_cop_config_extra(raw))
          entry['other_cops'] = merge_peers(effective_peer_overrides, entry['other_cops'])
          CAPTURES << entry
          @__last_entry = entry
          result
        end
      end

      # Tests that never reach an assertion (a raised error, or direct cop-internals access)
      # are recorded as uncaptured for the skip report.
      module CaptureLifecycle
        def before_setup
          @__captures_before = CAPTURES.size
          @__last_entry = nil
          PEER_READS.clear
          super
        end

        def after_teardown
          super
          return if skipped? || CAPTURES.size != @__captures_before

          reason = failure ? failure.message.lines.first&.chomp : 'did not call assert_offense/assert_no_offenses'
          CAPTURES.push('kind' => 'uncaptured', 'path' => port_path, 'order' => port_order, 'reason' => reason)
        end
      end

      RuboCop::Config.prepend(RecordPeerReads)
      RuboCop::Minitest::AssertOffense.prepend(CaptureAssertion)
      Minitest::Test.prepend(CaptureLifecycle)
      Minitest.after_run do
        CAPTURES.replace(CAPTURES.each_with_index.sort_by { |entry, index| [entry['order'], index] }.map(&:first))
        write_captures
      end
    RUBY
  else
    helper << <<~RUBY
      # RSpec suites (RuboCop itself, rubocop-rails, rubocop-performance,
      # rubocop-thread_safety): RuboCop's `ExpectOffense` helpers.
      module CaptureOffense
        include CaptureConfig

        def current_path
          self.class.parent_groups.reverse.map(&:description) + [RSpec.current_example.description]
        end

        # Specs that build their own `subject(:cop)` without the shared `config`
        # context (e.g. Style::NegatedIf/NegatedUnless) define neither `cop_class`
        # nor `cop_config`.
        def port_cop_class
          respond_to?(:cop_class) ? cop_class : described_class
        end

        def port_cop_config
          respond_to?(:cop_config) ? cop_config : {}
        end

        def port_other_cops
          respond_to?(:other_cops) ? other_cops : {}
        end

        # rubocop-rails' 'with SchemaLoader' shared context: the example's `let(:schema)`, the
        # `db/schema.rb` text it stubs in, or nil when the example runs without one.
        def port_schema
          respond_to?(:schema_path) && respond_to?(:schema) ? schema : nil
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
            'file' => fixture_file(file),
            'cop_config' => raw,
            'other_cops' => port_other_cops,
            'gem_versions' => port_gem_versions,
            'ruby_version' => ruby_version,
            'schema' => port_schema
          }
          result = super
          entry['cop_config'] = raw.merge(effective_cop_config_extra(raw))
          entry['other_cops'] = merge_peers(effective_peer_overrides, entry['other_cops'])
          entry['offenses'] = injected_offenses
          entry['display_cop_names'] = true if port_display_cop_names?
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
              'other_cops' => merge_peers(effective_peer_overrides, port_other_cops),
              'offenses' => injected_offenses,
              'gem_versions' => port_gem_versions,
              'ruby_version' => ruby_version,
              'schema' => port_schema,
              'annotated' => annotated
            }
            entry['display_cop_names'] = true if port_display_cop_names?
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
            'file' => fixture_file(file),
            'source' => source,
            'cop_config' => raw,
            'other_cops' => port_other_cops,
            'gem_versions' => port_gem_versions,
            'ruby_version' => ruby_version,
            'schema' => port_schema
          }
          result = super
          entry['cop_config'] = raw.merge(effective_cop_config_extra(raw))
          entry['other_cops'] = merge_peers(effective_peer_overrides, entry['other_cops'])
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
        # An extension's spec_helper may shuffle examples (case naming is order-dependent) and
        # persist example status under its own tree; neither may happen here.
        config.order = :defined
        config.example_status_persistence_file_path = nil

        config.before(:suite) { RuboCop::Config.prepend(RecordPeerReads) }
        config.before(:each) do
          @__captures_before = CAPTURES.size
          PEER_READS.clear
        end
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

      at_exit { write_captures }
    RUBY
  end
  File.write(helper_path, helper)

  env = { 'BUNDLE_GEMFILE' => File.join(work, 'Gemfile') }
  command, chdir =
    if extension
      # From the scratch directory, so the suite cannot write into --gem-src (simplecov's
      # coverage/, rspec's example status file); --gem-src's lib/ shadows the installed gem.
      load_path = ['-I', File.join(gem_src, 'lib'), '-I', File.dirname(File.join(gem_src, extension.support))]
      runner = framework == :minitest ? ['ruby'] : ['rspec']
      [[*runner, *load_path, '-r', helper_path, spec_file], work]
    else
      [['rspec', '-r', helper_path, File.basename(spec_file)], File.dirname(spec_file)]
    end
  run_out = IO.popen(env, ['bundle', 'exec', *command], chdir: chdir, err: [:child, :out]) { |io| io.read }
  puts run_out.lines.last(15).join

  abort "captures file was not written (#{framework} crashed before the captures were saved)" unless File.exist?(captures_path)

  captures = JSON.parse(File.read(captures_path))

  # --- fixture generation ----------------------------------------------

  uncaptured = captures.select { |c| c['kind'] == 'uncaptured' }
  captured = captures.reject { |c| c['kind'] == 'uncaptured' }

  min_prism_ruby = 3.3
  # CopHelper's let(:ruby_version) resolves to 3.3 when PARSER_ENGINE=parser_prism (set
  # above); rubocop-minitest's AssertOffense#ruby_version (what it parses with) to 3.4.
  default_ruby_version = extension ? extension.default_ruby_version : 3.3

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
    when String
      scalar = YAML.dump(v).sub(/\A---\s*/, '').chomp
      # Psych emits a plain scalar whenever block context allows it, but these
      # values land inside a flow sequence/mapping where `[]{},` are indicators.
      scalar.match?(/\A[^'"].*[\[\]{},]/m) ? JSON.generate(v) : scalar
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
    lead = case_prefix.empty? ? [] : [case_prefix]
    name = (lead + parts + [desc_slug]).join('_')
    while name.length > 60 && !parts.empty?
      parts.shift
      name = (lead + parts + [desc_slug]).join('_')
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
    File.write(File.join(out_dir, "#{name}.schema.rb"), c['schema']) if c['schema']
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
    all_cops['DisplayCopNames'] = true if c['display_cop_names']
    file_comment = c['file']

    gem_versions = c['gem_versions'] || {}

    next if cop_config.empty? && other_cops.empty? && all_cops.empty? && !file_comment && gem_versions.empty?

    yml_lines = []
    yml_lines << "# file: #{file_comment}" if file_comment
    yml_lines << "# gem_versions: #{gem_versions.map { |gem_name, version| "#{gem_name}=#{version}" }.join(', ')}" unless gem_versions.empty?
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
