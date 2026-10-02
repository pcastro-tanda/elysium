# Style/RedundantRegexpConstructor

Checks for the instantiation of regexp using redundant `Regexp.new` or `Regexp.compile`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for the instantiation of a regexp using a redundant `Regexp.new` or `Regexp.compile`. Autocorrect replaces it with a regexp literal which is the simplest and fastest.

## Options

This rule has no options.

## Blind spots

None recorded.
