# Performance/CompareWithBlock

Use `sort_by(&:foo)` instead of `sort { |a, b| a.foo <=> b.foo }`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `sort { |a, b| a.foo <=> b.foo }` can be replaced by `sort_by(&:foo)`. This cop also checks `sort!`, `min`, `max` and `minmax` methods.

```ruby
# bad
array.sort { |a, b| a.foo <=> b.foo }
array.sort { |a, b| a[:foo] <=> b[:foo] }

# good
array.sort_by(&:foo)
array.sort_by { |a| a[:foo] }
```

## Options

This rule has no options.

## Blind spots

None recorded.
