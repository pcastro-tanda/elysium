# Minitest/AssertSilent

This cop enforces the test to use `assert_silent { ... }` instead of using `assert_output('', '') { ... }`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `assert_silent { ... }` instead of using `assert_output('', '') { ... }`.

```ruby
# bad
assert_output('', '') { puts object.do_something }

# good
assert_silent { puts object.do_something }
```

## Options

This rule has no options.

## Blind spots

None recorded.
