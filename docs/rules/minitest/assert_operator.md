# Minitest/AssertOperator

Enforces the use of `assert_operator(expected, :<, actual)` over `assert(expected < actual)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of `assert_operator(expected, :<, actual)` over `assert(expected < actual)`.

```ruby
# bad
assert(expected < actual)

# good
assert_operator(expected, :<, actual)
```

## Options

This rule has no options.

## Blind spots

None recorded.
