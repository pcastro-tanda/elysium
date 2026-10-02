# Style/EmptyStringInsideInterpolation

Checks for empty strings being assigned inside string interpolation.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |



## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `trailing_conditional` | `trailing_conditional`, `ternary` | Whether an empty-string branch should become a trailing modifier conditional or stay a ternary. |

## Blind spots

If a block-form `if`/`unless` has an empty primary or else branch but the other side is entirely absent (no `else` at all, matching upstream's own unguarded `child_node.else_branch.source`/`child_node.if_branch.source` calls), upstream would raise `NoMethodError` on `nil`; this port instead silently skips the correction rather than panicking.
