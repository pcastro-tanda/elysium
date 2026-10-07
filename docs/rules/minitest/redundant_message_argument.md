# Minitest/RedundantMessageArgument

Detects redundant message argument in assertion methods.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Detects redundant message argument in assertion methods. The message argument `nil` is redundant because it is the default value.

```ruby
# bad
assert_equal(expected, actual, nil)

# good
assert_equal(expected, actual)
assert_equal(expected, actual, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.
