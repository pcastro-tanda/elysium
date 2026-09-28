# Lint/SafeNavigationChain

Do not chain ordinary method call after safe navigation operator.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

The safe navigation operator returns nil if the receiver is nil. If you chain an ordinary method call after a safe navigation operator, it raises NoMethodError. We should use a safe navigation operator after a safe navigation operator.
This cop checks for the problem outlined above.

## Options

This rule has no options.

## Blind spots

None recorded.
