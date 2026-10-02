# Lint/RedundantTypeConversion

Checks for redundantly converting a literal to the same type.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for redundant uses of `to_s`, `to_sym`, `to_i`, `to_f`, `to_d`, `to_r`, `to_c`, `to_a`, `to_h`, and `to_set`.

When one of these methods is called on an object of the same type, that object is returned, making the call unnecessary.

## Options

This rule has no options.

## Blind spots

None recorded.
