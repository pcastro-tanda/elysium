# Style/ReturnNil

Use return instead of return nil.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces consistency between `return nil` and `return`. This cop is disabled by default. Because there seems to be a perceived semantic difference between `return` and `return nil`. The former can be seen as just halting evaluation, while the latter might be used when the return value is of specific concern.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `return` | `return`, `return_nil` | Whether to prefer `return` or `return nil`. |

## Blind spots

None recorded.
