# Style/ExactRegexpMatch

Checks for exact regexp match inside Regexp literals.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |



## Options

This rule has no options.

## Blind spots

The upstream `regexp_parser`-based tokenizer is reimplemented as a direct scan for the single `\A<literal>\z` shape (see the module doc); regexes regexp_parser itself fails to parse (e.g. an incomplete `\P` property escape) are treated the same as any other non-matching shape (no offense) rather than needing a distinct parse-failure path.
