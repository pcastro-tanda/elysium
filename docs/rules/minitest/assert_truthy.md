# Minitest/AssertTruthy

This cop enforces the test to use `assert(actual)` instead of using `assert_equal(true, actual)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Enforces the test to use `assert(actual)` instead of using `assert_equal(true, actual)`.

This cop is unsafe because true might be expected instead of truthy. False positives cannot be prevented when this is a variable or method return value.

```ruby
# bad
assert_equal(true, actual)
assert_equal(true, actual, 'message')

# good
assert(actual)
assert(actual, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.
