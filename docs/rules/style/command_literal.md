# Style/CommandLiteral

Use `` or %x around command literals.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces using `` or %x around command literals.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `backticks` | `backticks`, `percent_x`, `mixed` | The preferred style for command literals. |
| AllowInnerBackticks | false |  | Whether an inner backtick is allowed to stay as a backtick literal instead of forcing `%x`. |

## Blind spots

None recorded.
