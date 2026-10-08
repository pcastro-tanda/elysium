# Minitest/AssertNil

This cop enforces the test to use `assert_nil` instead of using `assert_equal(nil, something)` or `assert(something.nil?)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `assert_nil` instead of using `assert_equal(nil, something)`, `assert(something.nil?)`, or `assert_predicate(something, :nil?)`.

```ruby
# bad
assert_equal(nil, actual)
assert_equal(nil, actual, 'message')
assert(object.nil?)
assert(object.nil?, 'message')
assert_predicate(object, :nil?)
assert_predicate(object, :nil?, 'message')

# good
assert_nil(actual)
assert_nil(actual, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.
