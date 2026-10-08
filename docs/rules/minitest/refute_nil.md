# Minitest/RefuteNil

This cop enforces the test to use `refute_nil` instead of using `refute_equal(nil, something)` or `refute(something.nil?)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `refute_nil` instead of using `refute_equal(nil, something)`, `refute(something.nil?)`, or `refute_predicate(something, :nil?)`.

```ruby
# bad
refute_equal(nil, actual)
refute_equal(nil, actual, 'message')
refute(object.nil?)
refute(object.nil?, 'message')
refute_predicate(object, :nil?)
refute_predicate(object, :nil?, 'message')

# good
refute_nil(actual)
refute_nil(actual, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.
