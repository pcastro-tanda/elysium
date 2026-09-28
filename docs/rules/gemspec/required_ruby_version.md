# Gemspec/RequiredRubyVersion

Checks that `required_ruby_version` of gemspec is specified and equal to `TargetRubyVersion` of .rubocop.yml.

| | |
| --- | --- |
| Department | Gemspec |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | nursery |

Checks that `required_ruby_version` of gemspec is specified and equal to `TargetRubyVersion`
of .rubocop.yml.

This ensures that RuboCop is using the same Ruby version as the gem.

```ruby
# When `TargetRubyVersion` of .rubocop.yml is `2.5`.

# bad
Gem::Specification.new do |spec|
  # no `required_ruby_version` specified
end

# bad
Gem::Specification.new do |spec|
  spec.required_ruby_version = '>= 2.4.0'
end

# good
Gem::Specification.new do |spec|
  spec.required_ruby_version = '>= 2.5.0'
end

# accepted but not recommended
Gem::Specification.new do |spec|
  spec.required_ruby_version = ['>= 2.5.0', '< 2.7.0']
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
