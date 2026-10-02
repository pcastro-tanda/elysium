# Lint/DuplicateRegexpCharacterClassElement

Checks for duplicate elements in Regexp character classes.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for duplicate elements in `Regexp` character classes.

```ruby
# bad
r = /[xyx]/

# bad
r = /[0-9x0-9]/

# good
r = /[xy]/

# good
r = /[0-9x]/
```

## Options

This rule has no options.

## Blind spots

`\c`/`\C-`/`\M-` control and meta escapes are consumed as opaque single elements without validating their internal syntax. A genuinely nested literal set (Oniguruma's `[a[bc]]` union form, as opposed to a POSIX bracket expression) is scanned as one atomic element at the outer level rather than separately walked for its own internal duplicates; no known fixture uses that rare form. Leading `]` right after `[`/`[^]` (a rare Oniguruma idiom for a class that contains a literal `]`) is treated as an ordinary close bracket, matching RuboCop's own spec suite.
