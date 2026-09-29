# Style/EndBlock

Avoid the use of END blocks.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for `END` blocks. `END` blocks are Perl-style constructs and `Kernel#at_exit` is the idiomatic Ruby alternative, as it's explicit and can be used anywhere.

## Options

This rule has no options.

## Blind spots

None recorded.
