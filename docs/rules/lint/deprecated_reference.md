# Lint/DeprecatedReference

Checks for references to methods and constants documented as deprecated with a YARD `@deprecated` tag. Requires `AllCops/UseProjectIndex` to be enabled.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | nursery |



## Options

This rule has no options.

## Blind spots

No project-wide index is available (no `rubydex` gem equivalent), so this cop never reports anything; it is a documented no-op, matching upstream's own "without the index the cop does nothing" behavior.
