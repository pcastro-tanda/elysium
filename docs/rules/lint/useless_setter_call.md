# Lint/UselessSetterCall

Checks for useless setter call to a local variable.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for setter call to local variable as the final expression of a function definition.

There are edge cases in which the local variable references a value that is also accessible outside the local scope. This is not detected by the cop, and it can yield a false positive. As well, autocorrection is unsafe because the method's return value will be changed.

## Options

This rule has no options.

## Blind spots

None recorded.
