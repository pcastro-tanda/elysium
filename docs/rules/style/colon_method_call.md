# Style/ColonMethodCall

Do not use :: for method call.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for methods invoked via the `::` operator instead of the `.` operator (like `FileUtils::rmdir` instead of `FileUtils.rmdir`). The `::` operator is conventionally used to reference constants, so using it for method calls can be misleading.

## Options

This rule has no options.

## Blind spots

None recorded.
