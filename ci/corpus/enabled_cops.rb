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

rules = File.readlines(ARGV.fetch(0), chomp: true).reject { |l| l.empty? || l.start_with?('#') }
config = RuboCop::ConfigStore.new.for_pwd
registry = RuboCop::Cop::Registry.global

enabled = rules.select do |name|
  cop = registry.find_by_cop_name(name)
  abort "error: RuboCop doesn't know #{name}" unless cop
  registry.enabled?(cop, config)
end

puts enabled.join(',')
