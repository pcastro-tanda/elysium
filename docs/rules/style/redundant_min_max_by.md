# Style/RedundantMinMaxBy

Identifies places where `max_by`/`min_by` can be replaced by `max`/`min`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `max_by { ... }`, `min_by { ... }`, or `minmax_by { ... }` can be replaced by `max`, `min`, or `minmax`.

## Options

This rule has no options.

## Blind spots

None recorded.
