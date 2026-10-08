# Performance/StartWith

Use `start_with?` instead of a regex match anchored to the beginning of a string.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies unnecessary use of a regex where `String#start_with?` would suffice.

This cop has a `SafeMultiline` option, `true` by default, because `^start` behaves differently from `start_with?` for multiline receivers.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| SafeMultiline | true |  | Do not flag `^` anchored regexps. |

## Blind spots

None recorded.
