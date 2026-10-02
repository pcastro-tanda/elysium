# Gemspec/AddRuntimeDependency

Prefer `add_dependency` over `add_runtime_dependency`.

| | |
| --- | --- |
| Department | Gemspec |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Prefer `add_dependency` over `add_runtime_dependency` as the latter is
considered soft-deprecated.

```ruby
# bad
Gem::Specification.new do |spec|
  spec.add_runtime_dependency('rubocop')
end

# good
Gem::Specification.new do |spec|
  spec.add_dependency('rubocop')
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
