# Gemspec/DuplicatedAssignment

An attribute assignment method calls should be listed only once in a gemspec.

| | |
| --- | --- |
| Department | Gemspec |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Assigning to an attribute with the same name using `spec.foo =` or
`spec.attribute#[]=` will be an unintended usage. On the other hand,
duplication of methods such as `spec.requirements`,
`spec.add_runtime_dependency`, and others are permitted because it is
the intended use of appending values.

```ruby
# bad
Gem::Specification.new do |spec|
  spec.name = 'rubocop'
  spec.name = 'rubocop2'
end

# good
Gem::Specification.new do |spec|
  spec.name = 'rubocop'
end

# good
Gem::Specification.new do |spec|
  spec.requirements << 'libmagick, v6.0'
  spec.requirements << 'A good graphics card'
end

# good
Gem::Specification.new do |spec|
  spec.add_dependency('parallel', '~> 1.10')
  spec.add_dependency('parser', '>= 2.3.3.1', '< 3.0')
end

# bad
Gem::Specification.new do |spec|
  spec.metadata["key"] = "value"
  spec.metadata["key"] = "value"
end

# good
Gem::Specification.new do |spec|
  spec.metadata["key"] = "value"
end
```

## Options

This rule has no options.

## Blind spots

Upstream's `match_block_variable_name?` `return`s from inside a `def_node_search` block, so
only the *first* `Gem::Specification.new do |x| ... end` block found anywhere in the file is
ever consulted for its parameter name; a file with two such blocks using different parameter
names only recognizes the first one's, a literal upstream quirk reproduced here rather than
fixed. `Include: ['**/*.gemspec']` restricts this cop to gemspec files upstream; elysium applies
that restriction at the config-file-matching layer (see `crates/config`), not in this rule's own
logic.
