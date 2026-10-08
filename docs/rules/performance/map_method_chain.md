# Performance/MapMethodChain

Checks if the `map` method is used in a chain.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks if the map method is used in a chain.

Autocorrection is not supported because an appropriate block variable name cannot be determined automatically.

This cop is unsafe because false positives occur if the number of times the first method is executed affects the return value of subsequent methods.

## Options

This rule has no options.

## Blind spots

None recorded.
