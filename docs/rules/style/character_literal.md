# Style/CharacterLiteral

Checks for uses of character literals.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of the character literal ?x.
Starting with Ruby 1.9 character literals are
essentially one-character strings, so this syntax
is mostly redundant at this point.

A `?` character literal can be used to express meta and control characters.
That's a good use case of a `?` literal so it doesn't count as an offense.

## Options

This rule has no options.

## Blind spots

None recorded.
