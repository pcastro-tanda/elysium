# Performance/ConstantRegexp

Finds regular expressions with dynamic components that are all constants.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Finds regular expressions with dynamic components that are all constants.

Ruby allocates a new Regexp object every time it executes a code containing such
a regular expression. It is more efficient to extract it into a constant,
memoize it, or add an `/o` option to perform `#{}` interpolation only once and
reuse that Regexp object.

## Options

This rule has no options.

## Blind spots

None recorded.
