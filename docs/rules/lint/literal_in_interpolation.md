# Lint/LiteralInInterpolation

Checks for literals used in interpolation.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for interpolated literals.

NOTE: Array literals interpolated in regexps are not handled by this cop,
but by `Lint/ArrayLiteralInRegexp` instead.

```ruby
# bad
"result is #{10}"

# good
"result is 10"
```

## Options

This rule has no options.

## Blind spots

None recorded.
