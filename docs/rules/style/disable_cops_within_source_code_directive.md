# Style/DisableCopsWithinSourceCodeDirective

Forbids disabling/enabling cops within source code.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Detects comments to enable/disable RuboCop.
This is useful if want to make sure that every RuboCop error gets fixed
and not quickly disabled with a comment.

Specific cops can be allowed with the `AllowedCops` configuration. Note that
if this configuration is set, `rubocop:disable all` is still disallowed.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedCops | `[]` |  | Cops that can be disabled/enabled by a directive comment. |

## Blind spots

None recorded.
