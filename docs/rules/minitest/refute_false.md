# Minitest/RefuteFalse

Check if your test uses `refute(actual)` instead of `assert_equal(false, actual)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Enforces the use of `refute(object)` over `assert_equal(false, object)`.

This cop is unsafe because it cannot detect failure when second argument is `nil`. False positives cannot be prevented when this is a variable or method return value.

```ruby
# bad
assert_equal(false, actual)
assert_equal(false, actual, 'message')

assert(!test)
assert(!test, 'message')

# good
refute(actual)
refute(actual, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.
