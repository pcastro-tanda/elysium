# Style/OneClassPerFile

Checks that each source file defines at most one top-level class or module.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Keeping one class or module per file makes it easier to find and navigate code, and follows the convention used by most Ruby projects.

Classes and modules listed in `AllowedClasses` are not counted toward the limit. This is useful for small ancillary classes like custom exception classes that logically belong with the main class.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedClasses | `[]` |  | Classes/modules not counted toward the one-per-file limit. |

## Blind spots

None recorded.
