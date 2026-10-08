# Minitest/RefuteEqual

Enforces the use of `refute_equal(expected, object)` over `assert(expected != actual)` or `assert(! expected == actual)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of `refute_equal(expected, object)` over `assert(expected != actual)` or `assert(! expected == actual)`.

```ruby
# bad
assert("rubocop-minitest" != actual)
refute("rubocop-minitest" == actual)
assert_operator("rubocop-minitest", :!=, actual)
refute_operator("rubocop-minitest", :==, actual)

# good
refute_equal("rubocop-minitest", actual)
```

## Options

This rule has no options.

## Blind spots

None recorded.
