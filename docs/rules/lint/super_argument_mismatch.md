# Lint/SuperArgumentMismatch

Checks for `super` calls that pass the wrong number of positional arguments to the overridden implementation, using the project index.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for `super` calls with explicit arguments that pass the wrong
number of positional arguments to the overridden implementation, using
the project index.

The check is powered by the project-wide index, so it only runs when
`AllCops/UseProjectIndex` is enabled and the `rubydex` gem is installed.
Without the index the cop does nothing.

## Options

This rule has no options.

## Blind spots

Not ported: `AllCops/UseProjectIndex` cross-file `super` arity checking (`ProjectIndexHelp`, `IndexedMethodArity`, method-resolution-order lookup) needs the `rubydex` gem's project index, which elysium has no equivalent of. This cop is always a no-op here, matching upstream's own documented behavior when the index is unavailable.
