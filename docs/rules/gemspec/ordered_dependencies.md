# Gemspec/OrderedDependencies

Dependencies in the gemspec should be alphabetically sorted.

| | |
| --- | --- |
| Department | Gemspec |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Dependencies in the gemspec should be alphabetically sorted.

```ruby
# bad
spec.add_dependency 'rubocop'
spec.add_dependency 'rspec'

# good
spec.add_dependency 'rspec'
spec.add_dependency 'rubocop'

# good
spec.add_dependency 'rubocop'

spec.add_dependency 'rspec'
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| TreatCommentsAsGroupSeparators | true |  | A comment directly above a dependency breaks up its group, so gems on either side of it are compared separately. |
| ConsiderPunctuation | false |  | By default, `-` and `_` are ignored for order purposes; set this to compare them too. |

## Blind spots

`Include: ['**/*.gemspec']` restricts this cop to gemspec files upstream; elysium applies that
restriction at the config-file-matching layer (see `crates/config`), not in this rule's own logic.
Matching the upstream `def_node_search`, declarations are searched for anywhere in the file, not
only inside a `Gem::Specification.new` block.
