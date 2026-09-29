# Style/PercentQLiterals

Checks if uses of %Q/%q match the configured preference.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for usage of the %Q() syntax when %q() would do.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `lower_case_q` | `lower_case_q`, `upper_case_q` | The preferred style for `%q`/`%Q` literals. |

## Blind spots

None recorded.
