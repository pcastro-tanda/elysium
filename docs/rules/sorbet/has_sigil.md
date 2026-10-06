# Sorbet/HasSigil

Makes the Sorbet typed sigil mandatory in all files.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Makes the Sorbet typed sigil mandatory in all files.

Options:

* `SuggestedStrictness`: Sorbet strictness level suggested in offense messages (default: 'false')
* `MinimumStrictness`: If set, make offense if the strictness level in the file is below this one

If a `SuggestedStrictness` level is specified, it will be used in autocorrect.
If a `MinimumStrictness` level is specified, it will be used in offense messages and autocorrect.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| SuggestedStrictness | `false` |  | Sorbet strictness level suggested in offense messages and used in autocorrect. |
| ExactStrictness | `nil` |  | If set, make offense if the strictness level in the file is different than this one. |
| MinimumStrictness | `nil` |  | If set, make offense if the strictness level in the file is below this one. |

## Blind spots

None recorded.
