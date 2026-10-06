# Rails/RefuteMethods

Use `assert_not` methods instead of `refute` methods.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Use `assert_not` methods instead of `refute` methods.

```ruby
# bad
refute false
refute_empty [1, 2, 3]

# good
assert_not false
assert_not_empty [1, 2, 3]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `assert_not` | `assert_not`, `refute` | Which family of methods to prefer. |

## Blind spots

None recorded.
