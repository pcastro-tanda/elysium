# Naming/HeredocDelimiterNaming

Use descriptive heredoc delimiters.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks that your heredocs are using meaningful delimiters. By default it disallows `END` and `EO*`, and can be configured through forbidden listing additional delimiters.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| ForbiddenDelimiters | `/(^|\s)(EO[A-Z]{1}|END)(\s|$)/i` |  | Heredoc delimiters matching any of these patterns are forbidden. |

## Blind spots

None recorded.
