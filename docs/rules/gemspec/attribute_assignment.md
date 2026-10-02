# Gemspec/AttributeAssignment

Use consistent style for Gemspec attributes assignment.

| | |
| --- | --- |
| Department | Gemspec |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Use consistent style for Gemspec attributes assignment.

```ruby
# bad
# This example uses two styles for assignment of metadata attribute.
Gem::Specification.new do |spec|
  spec.metadata = { 'key' => 'value' }
  spec.metadata['another-key'] = 'another-value'
end

# good
Gem::Specification.new do |spec|
  spec.metadata['key'] = 'value'
  spec.metadata['another-key'] = 'another-value'
end

# good
Gem::Specification.new do |spec|
  spec.metadata = { 'key' => 'value', 'another-key' => 'another-value' }
end
```

## Options

This rule has no options.

## Blind spots

Upstream's `match_block_variable_name?` `return`s from inside a `def_node_search` block, so
only the *first* `Gem::Specification.new do |x| ... end` block found anywhere in the file is
ever consulted for its parameter name; a literal upstream quirk reproduced here rather than
fixed. `Include: ['**/*.gemspec']` restricts this cop to gemspec files upstream; elysium applies
that restriction at the config-file-matching layer, not in this rule's own logic.
