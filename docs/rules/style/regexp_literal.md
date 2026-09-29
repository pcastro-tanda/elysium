# Style/RegexpLiteral

Use / or %r around regular expressions.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces using `//` or `%r` around regular expressions.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `slashes` | `slashes`, `percent_r`, `mixed` | The preferred style for regular expression literals. |
| AllowInnerSlashes | false |  | Whether an inner unescaped slash is allowed to stay in a slash literal instead of forcing `%r`. |

## Blind spots

None recorded.
