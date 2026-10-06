# Minitest/RefuteSame

Enforces the use of `refute_same(expected, actual)` over `refute(expected.equal?(actual))`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of `refute_same(expected, actual)` over `refute(expected.equal?(actual))`.

Use `refute_same` only when there is a need to compare by identity. Otherwise, use `refute_equal`.

```ruby
# bad
refute(expected.equal?(actual))
refute_equal(expected.object_id, actual.object_id)

# good
refute_same(expected, actual)
```

## Options

This rule has no options.

## Blind spots

None recorded.
