# Lint/InheritException

Avoid inheriting from the `Exception` class.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Looks for error classes inheriting from `Exception`. It is configurable to suggest using either `StandardError` (default) or `RuntimeError` instead.

This cop's autocorrection is unsafe because `rescue` that omit exception class handle `StandardError` and its subclasses, but not `Exception` and its subclasses.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `standard_error` | `standard_error`, `runtime_error` | The preferred base class in favour of `Exception`. |

## Blind spots

None recorded.
