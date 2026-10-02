# Lint/NameTypo

Checks for probable typos in constant and method names, using the project index.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for probable typos in constant and method names: a name that
does not resolve anywhere in the project, used in a namespace the
project does define, with a close-named sibling to suggest instead.

The check is powered by the project-wide index, so it only runs when
`AllCops/UseProjectIndex` is enabled and the `rubydex` gem is installed.
Without the index the cop does nothing.

## Options

This rule has no options.

## Blind spots

Not ported: `AllCops/UseProjectIndex` cross-file typo detection (`ProjectIndexHelp`, `did_you_mean`-based suggestions) needs the `rubydex` gem's project index, which elysium has no equivalent of. This cop is always a no-op here, matching upstream's own documented behavior when the index is unavailable.
