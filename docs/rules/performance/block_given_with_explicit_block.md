# Performance/BlockGivenWithExplicitBlock

Check block argument explicitly instead of using `block_given?`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies unnecessary use of a `block_given?` where explicit check of block argument would suffice.

NOTE: This cop produces code with significantly worse performance when a block is being passed to the method and as such should not be enabled.

## Options

This rule has no options.

## Blind spots

None recorded.
