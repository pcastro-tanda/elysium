# Minitest/RefuteOperator

Enforces the use of `refute_operator(expected, :<, actual)` over `refute(expected < actual)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of `refute_operator(expected, :<, actual)` over `refute(expected < actual)`.

```ruby
# bad
refute(expected < actual)

# good
refute_operator(expected, :<, actual)
```

## Options

This rule has no options.

## Blind spots

None recorded.
