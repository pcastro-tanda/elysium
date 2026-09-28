# Lint/EmptyInterpolation

Checks for empty interpolation.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

```ruby
# bad
"result is #{}"

# good
"result is #{some_result}"
```

## Options

This rule has no options.

## Blind spots

None recorded.
