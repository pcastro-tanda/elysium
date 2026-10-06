# Performance/ReverseEach

Use `reverse_each` instead of `reverse.each`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies usages of `reverse.each` and changes them to use `reverse_each` instead. If the return value is used, it will not be detected because the result will be different.

## Options

This rule has no options.

## Blind spots

None recorded.
