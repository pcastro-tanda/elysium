# Performance/ReverseFirst

Use `last(n).reverse` instead of `reverse.first(n)`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `reverse.first(n)` and `reverse.first` can be replaced by `last(n).reverse` and `last`.

```ruby
# bad
array.reverse.first(5)
array.reverse.first

# good
array.last(5).reverse
array.last
```

## Options

This rule has no options.

## Blind spots

None recorded.
