# Lint/OrAssignmentToConstant

Checks unintended or-assignment to constant.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Constants should always be assigned in the same location. And its value
should always be the same. If constants are assigned in multiple
locations, the result may vary depending on the order of `require`.

@safety
  This cop is unsafe because code that is already conditionally
  assigning a constant may have its behavior changed by autocorrection.

```ruby
# bad
CONST ||= 1

# good
CONST = 1
```

## Options

This rule has no options.

## Blind spots

None recorded.
