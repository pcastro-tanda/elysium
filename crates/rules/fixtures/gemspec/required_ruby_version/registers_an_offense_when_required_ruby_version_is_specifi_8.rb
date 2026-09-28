Gem::Specification.new do |spec|
  spec.required_ruby_version = '~> 3'
                               ^^^^^^ `required_ruby_version` and `TargetRubyVersion` (3.4, which may be specified in .rubocop.yml) should be equal.
end
