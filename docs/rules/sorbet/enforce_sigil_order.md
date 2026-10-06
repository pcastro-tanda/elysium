# Sorbet/EnforceSigilOrder

Ensures that Sorbet sigil comes first in a file.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that the Sorbet sigil comes as the first magic comment in the file, after the encoding comment if any.

The expected order for magic comments is: (en)?coding, typed, warn_indent then frozen_string_literal.

The ordering is for consistency only, except for the encoding comment which must be first, if present.

Only `(en)?coding`, `typed`, `warn_indent` and `frozen_string_literal` magic comments are considered, other comments or magic comments are left in the same place.

## Options

This rule has no options.

## Blind spots

None recorded.
