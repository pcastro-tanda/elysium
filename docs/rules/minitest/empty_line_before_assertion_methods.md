# Minitest/EmptyLineBeforeAssertionMethods

Add empty line before assertion methods.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces empty line before assertion methods because it separates assertion phase.

```ruby
# bad
do_something
assert_equal(expected, actual)

# good
do_something

assert_equal(expected, actual)
```

## Options

This rule has no options.

## Blind spots

None recorded.
