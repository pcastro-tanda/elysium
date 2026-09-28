# Lint/UnifiedInteger

Use Integer instead of Fixnum or Bignum.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for using Fixnum or Bignum constant.

# bad
1.is_a?(Fixnum)
1.is_a?(Bignum)

# good
1.is_a?(Integer)


## Options

This rule has no options.

## Blind spots

None recorded.
