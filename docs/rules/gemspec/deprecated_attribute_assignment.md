# Gemspec/DeprecatedAttributeAssignment

Checks that deprecated attributes are not set in a gemspec file.

| | |
| --- | --- |
| Department | Gemspec |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks that deprecated attributes are not set in a gemspec file.
Removing deprecated attributes allows the user to receive smaller packed
gems.

```ruby
# bad
Gem::Specification.new do |spec|
  spec.name = 'your_cool_gem_name'
  spec.test_files = Dir.glob('test/**/*')
end

# bad
Gem::Specification.new do |spec|
  spec.name = 'your_cool_gem_name'
  spec.test_files += Dir.glob('test/**/*')
end

# good
Gem::Specification.new do |spec|
  spec.name = 'your_cool_gem_name'
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
