# Sorbet/EnforceSingleSigil

Ensures that there is only one Sorbet sigil in a file.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that there is only one Sorbet sigil in a given file.

The first sigil encountered represents the "real" strictness, so the following ones are removed by autocorrect. Other comments or magic comments are left in place.

## Options

This rule has no options.

## Blind spots

None recorded.
