# Style/MagicCommentFormat

Use a consistent style for magic comments.

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
| EnforcedStyle | `snake_case` | `snake_case`, `kebab_case` | Separator style for magic comments. |
| DirectiveCapitalization | `lowercase` | `lowercase`, `uppercase` | Required capitalization for magic comment directives. |
| ValueCapitalization | `nil` | `lowercase`, `uppercase` | Required capitalization for magic comment values. |

## Blind spots

Validity (whether a comment counts as a magic comment at all) is approximated by the same keyword-scan used for format checking, rather than upstream's separate anchored `SimpleComment`/`EmacsComment` parse; a comment with extraneous text surrounding an otherwise-valid `keyword: value` substring would be accepted here but rejected upstream. An explicit YAML `DirectiveCapitalization: ~`/`ValueCapitalization: ~` override and the key being entirely absent both surface as `None` here (the config loader's `unset_nil` merge deletes an explicitly nulled key outright), so `DirectiveCapitalization`'s documented `lowercase` schema default can never actually take effect when the key is unset only because every layer omitted it.
