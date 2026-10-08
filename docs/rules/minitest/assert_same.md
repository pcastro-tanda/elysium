# Minitest/AssertSame

Enforces the use of `assert_same(expected, actual)` over `assert(expected.equal?(actual))`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of `assert_same(expected, actual)` over `assert(expected.equal?(actual))`.

Use `assert_same` only when there is a need to compare by identity. Otherwise, use `assert_equal`.

```ruby
# bad
assert(expected.equal?(actual))
assert_equal(expected.object_id, actual.object_id)

# good
assert_same(expected, actual)
```

## Options

This rule has no options.

## Blind spots

None recorded.
