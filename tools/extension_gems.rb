# frozen_string_literal: true

# Development/CI tool only. The RuboCop extension gems elysium ports, each
# pinned the way RuboCop itself is: its cops live in `crates/rules_<key>`,
# its `config/default.yml` is vendored verbatim there, and its fixtures (in
# `crates/rules/fixtures/<key>/`) come only from that version's own specs or
# tests, through tools/port_spec.rb. tools/check_fixtures.rb and CI's
# "matches pinned RuboCop" job verify both against a checkout of the tag.
#
# Run directly, it prints one line per gem for CI:
#
#   ruby tools/extension_gems.rb
#   # KEY GEM VERSION REPOSITORY TAG VENDORED_DEFAULT_YML

module ExtensionGems
  ROOT = File.expand_path('..', __dir__)

  # One extension gem. `framework` is how its suite asserts offenses:
  # `:rspec` (`expect_offense`, RuboCop's `CopHelper`) or `:minitest`
  # (rubocop-minitest's `AssertOffense#assert_offense`). `support` is the
  # suite's own spec_helper/test_helper, loaded before the capture hooks.
  # `gemfile` lists the suite's own dependencies beyond rubocop and the gem
  # itself (from the gem's Gemfile, at its Gemfile.lock version where a newer
  # one breaks the suite), resolved with `bundle lock --local`.
  # `default_ruby_version` is the target Ruby version an example runs at
  # unless it says otherwise, under `PARSER_ENGINE=parser_prism`: RuboCop's
  # `CopHelper` gives 3.3, rubocop-minitest's `AssertOffense` 3.4.
  Extension = Struct.new(
    :key, :gem, :version, :repository, :department, :framework, :version_file,
    :support, :spec_root, :gemfile,
    keyword_init: true
  ) do
    def tag
      "v#{version}"
    end

    def crate
      "rules_#{key}"
    end

    def crate_src
      File.join(ROOT, 'crates', crate, 'src')
    end

    def vendored_default_yml
      File.join(ROOT, 'crates', crate, gem, 'default.yml')
    end

    def default_ruby_version
      framework == :minitest ? 3.4 : 3.3
    end

    # The version a source checkout holds, read the way the gem reads it.
    def version_of(src)
      path = File.join(src, version_file)
      return nil unless File.exist?(path)

      text = File.read(path)
      version_file.end_with?('.rb') ? text[/STRING\s*=\s*'([^']+)'/, 1] : text.strip
    end

    # `explicit[key]` (from `--gem-src`), else a `<gem>-<version>` checkout
    # next to the RuboCop source (the corpus layout), else nil.
    def source(explicit, rubocop_src)
      return File.expand_path(explicit[key]) if explicit[key]

      sibling = File.join(File.dirname(File.expand_path(rubocop_src)), "#{gem}-#{version}")
      Dir.exist?(sibling) ? sibling : nil
    end
  end

  ALL = [
    Extension.new(
      key: 'rails', gem: 'rubocop-rails', version: '2.38.0',
      repository: 'https://github.com/rubocop/rubocop-rails.git', department: 'Rails',
      framework: :rspec, version_file: 'lib/rubocop/rails/version.rb',
      support: 'spec/spec_helper.rb', spec_root: 'spec/rubocop/cop', gemfile: ["gem 'rspec'"]
    ),
    Extension.new(
      key: 'performance', gem: 'rubocop-performance', version: '1.27.0',
      repository: 'https://github.com/rubocop/rubocop-performance.git', department: 'Performance',
      framework: :rspec, version_file: 'lib/rubocop/performance/version.rb',
      support: 'spec/spec_helper.rb', spec_root: 'spec/rubocop/cop', gemfile: ["gem 'rspec'"]
    ),
    Extension.new(
      key: 'thread_safety', gem: 'rubocop-thread_safety', version: '0.8.0',
      repository: 'https://github.com/rubocop/rubocop-thread_safety.git', department: 'ThreadSafety',
      framework: :rspec, version_file: 'lib/rubocop/thread_safety/version.rb',
      support: 'spec/spec_helper.rb', spec_root: 'spec/rubocop/cop',
      gemfile: ["gem 'rspec'", "gem 'simplecov'"]
    ),
    Extension.new(
      key: 'minitest', gem: 'rubocop-minitest', version: '0.40.0',
      repository: 'https://github.com/rubocop/rubocop-minitest.git', department: 'Minitest',
      framework: :minitest, version_file: 'lib/rubocop/minitest/version.rb',
      support: 'test/test_helper.rb', spec_root: 'test/rubocop/cop',
      gemfile: ["gem 'minitest', '~> 5.11'", "gem 'minitest-proveit'"]
    ),
    # rubocop-sorbet's tests build their cops themselves (`@cop = Foo.new`,
    # `target_cop.new(cop_config(...))`) on rubocop-minitest 0.38.1's
    # helpers, the version its Gemfile.lock pins: 0.40's `AssertOffense`
    # builds a config through the test's `cop_config`, which rubocop-sorbet's
    # test_helper redefines to need a `target_cop` most tests lack.
    Extension.new(
      key: 'sorbet', gem: 'rubocop-sorbet', version: '0.16.0',
      repository: 'https://github.com/Shopify/rubocop-sorbet.git', department: 'Sorbet',
      framework: :minitest, version_file: 'VERSION',
      support: 'test/test_helper.rb', spec_root: 'test/rubocop/cop',
      gemfile: ["gem 'minitest'", "gem 'rubocop-minitest', '0.38.1'", "gem 'mocha'", "gem 'debug'"]
    )
  ].freeze

  module_function

  def for_department(department)
    ALL.find { |extension| extension.department == department }
  end

  def for_key(key)
    ALL.find { |extension| extension.key == key }
  end

  # `rails=PATH,minitest=PATH` -> { 'rails' => 'PATH', 'minitest' => 'PATH' }.
  def parse_sources(text)
    text.split(',').to_h do |pair|
      key, path = pair.split('=', 2)
      abort "--gem-src expects KEY=PATH, got #{pair.inspect}" unless key && path && !path.empty?
      abort "--gem-src: unknown gem key #{key.inspect} (one of #{ALL.map(&:key).join(', ')})" unless for_key(key)

      [key, path]
    end
  end

  # RuboCop's file naming for a CamelCase cop or department name, acronyms
  # included: `HTTPStatus` -> `http_status`, `ThreadSafety` -> `thread_safety`.
  def snake(name)
    name.gsub(/([A-Z]+)([A-Z][a-z])/, '\1_\2').gsub(/([a-z\d])([A-Z])/, '\1_\2').downcase
  end
end

if $PROGRAM_NAME == __FILE__
  ExtensionGems::ALL.each do |extension|
    puts [extension.key, extension.gem, extension.version, extension.repository, extension.tag,
          extension.vendored_default_yml.delete_prefix("#{ExtensionGems::ROOT}/")].join(' ')
  end
end
