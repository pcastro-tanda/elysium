# Lint/AssignmentInCondition

Don't use assignment in conditions.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

`AllowSafeAssignment` option for safe assignment. By safe assignment we mean putting parentheses around an assignment to indicate "I know I'm using an assignment as a condition. It's not a mistake."

This cop's autocorrection is unsafe because it assumes that the author meant to use an assignment result as a condition.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowSafeAssignment | true |  | Whether an assignment wrapped in parentheses (e.g. `if (test = 10)`) is allowed. |

## Blind spots

None recorded.
