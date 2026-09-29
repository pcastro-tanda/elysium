# Style/ArrayJoin

Use Array#join instead of Array#*.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of `*` as a substitute for `Array#join`. Using `join` is clearer about intent and more readable than overloading the `*` operator for string conversion.

Not all cases can be reliably checked, due to Ruby's dynamic types, so we consider only cases when the first argument is an array literal or the second is a string literal.

## Options

This rule has no options.

## Blind spots

None recorded.
