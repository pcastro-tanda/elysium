# Minitest/LiteralAsActualArgument

This cop enforces correct order of `expected` and `actual` arguments for `assert_equal`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces correct order of expected and actual arguments for `assert_equal`.

```ruby
# bad
assert_equal foo, 2
assert_equal foo, [1, 2]
assert_equal foo, [1, 2], 'message'

# good
assert_equal 2, foo
assert_equal [1, 2], foo
assert_equal [1, 2], foo, 'message'
```

## Options

This rule has no options.

## Blind spots

None recorded.
