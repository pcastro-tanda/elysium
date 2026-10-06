# ThreadSafety/DirChdir

Avoid using `Dir.chdir` due to its process-wide effect.

| | |
| --- | --- |
| Department | ThreadSafety |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Avoid using `Dir.chdir` due to its process-wide effect. If `AllowCallWithBlock` (disabled by default) option is enabled, calling `Dir.chdir` with block will be allowed.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowCallWithBlock | false |  | Allow `Dir.chdir` and friends when called with a block. |

## Blind spots

None recorded.
