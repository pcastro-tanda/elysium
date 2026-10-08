# Performance/SortReverse

Use `sort.reverse` instead of `sort { |a, b| b <=> a }`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `sort { |a, b| b <=> a }` can be replaced by a
faster `sort.reverse`.

```ruby
# bad
array.sort { |a, b| b <=> a }

# good
array.sort.reverse
```

## Options

This rule has no options.

## Blind spots

None recorded.
