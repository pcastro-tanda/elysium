# Lint/MultipleComparison

Use `&&` operator to compare multiple values.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

In math and Python, we can use `x < y < z` style comparison to compare
multiple value. However, we can't use the comparison in Ruby. However,
the comparison is not syntax error. This cop checks the bad usage of
comparison operators.

```ruby
# bad
x < y < z
10 <= x <= 20

# good
x < y && y < z
10 <= x && x <= 20
```

## Options

This rule has no options.

## Blind spots

None recorded.
