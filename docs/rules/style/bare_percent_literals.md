# Style/BarePercentLiterals

Checks if usage of %() or %Q() matches configuration.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks if usage of `%()` or `%Q()` matches configuration. Consistent use of one style makes the codebase easier to read.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `bare_percent` | `percent_q`, `bare_percent` | The preferred style for bare `%()` vs. `%Q()` literals. |

## Blind spots

None recorded.
