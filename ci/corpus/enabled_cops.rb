# frozen_string_literal: true

# Prints, comma-separated, the cops from the given rules file that the
# current directory's RuboCop config actually enables (department-level
# `Enabled: false`, `pending` + `NewCops`, and inherited plugin configs all
# resolved by RuboCop itself). run.sh passes the result to `--only`, which
# would otherwise force-enable cops the app turns off and inflate the
# comparison with offenses the app never sees.
#
# Usage (from the app checkout, inside its bundle):
#   bundle exec ruby enabled_cops.rb path/to/rules.txt

require 'rubocop'

# Extension gems by department, loaded the way `rubocop --plugin <gem>` does
# (before the config is read) so their cops are registered and their
# `config/default.yml` is layered in even when the app's own config never
# names them. run.sh passes the same `--plugin`s to RuboCop. Keep in sync with
# `plugins_for` in run.sh and `EXTENSION_GEMS` in crates/xtask.
EXTENSION_GEMS = {
  'Rails' => 'rubocop-rails',
  'Performance' => 'rubocop-performance',
  'Minitest' => 'rubocop-minitest',
  'Sorbet' => 'rubocop-sorbet',
  'ThreadSafety' => 'rubocop-thread_safety'
}.freeze

rules = File.readlines(ARGV.fetch(0), chomp: true).reject { |l| l.empty? || l.start_with?('#') }
rules.map { |name| EXTENSION_GEMS[name.split('/').first] }.compact.uniq.each do |gem_name|
  RuboCop::ConfigLoaderResolver.new.resolve_plugins(RuboCop::Config.new, gem_name)
end

config = RuboCop::ConfigStore.new.for_pwd
registry = RuboCop::Cop::Registry.global

enabled = rules.select do |name|
  cop = registry.find_by_cop_name(name)
  abort "error: RuboCop doesn't know #{name}" unless cop
  registry.enabled?(cop, config)
end

puts enabled.join(',')
