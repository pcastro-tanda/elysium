require 'rubocop/version'

Gem::Specification.new do |spec|
  spec.version = RuboCop::Version::STRING
  spec.version = RuboCop::Version::STRING
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `version=` method calls already given on line 4 of the gemspec.
end
