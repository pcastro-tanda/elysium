# Minitest/AssertEqual

Enforces the use of `assert_equal(expected, actual)` over `assert(expected == actual)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of `assert_equal(expected, actual)` over `assert(expected == actual)`.

```ruby
# bad
assert("rubocop-minitest" == actual)
assert_operator("rubocop-minitest", :==, actual)

# good
assert_equal("rubocop-minitest", actual)
```

## Options

This rule has no options.

## Blind spots

None recorded.
