# Style/ReverseFind

Use `array.rfind` instead of `array.reverse.find`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

This cop is unsafe because it cannot be guaranteed that the receiver is an `Array` or responds to the replacement method.

## Options

This rule has no options.

## Blind spots

None recorded.
