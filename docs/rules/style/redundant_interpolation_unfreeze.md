# Style/RedundantInterpolationUnfreeze

Checks for redundant unfreezing of interpolated strings.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Before Ruby 3.0, interpolated strings followed the frozen string literal magic comment which sometimes made it necessary to explicitly unfreeze them. Ruby 3.0 changed interpolated strings to always be unfrozen which makes unfreezing them redundant.

## Options

This rule has no options.

## Blind spots

None recorded.
