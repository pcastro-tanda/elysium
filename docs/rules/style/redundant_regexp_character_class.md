# Style/RedundantRegexpCharacterClass

Checks for unnecessary single-element `Regexp` character classes.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

```ruby
# bad
r = /[x]/

# good
r = /x/

# bad
r = /[\s]/

# good
r = /\s/

# bad
r = %r{/[b]}

# good
r = %r{/b}

# good
r = /[ab]/
```

## Options

This rule has no options.

## Blind spots

`\c`/`\C-`/`\M-` control and meta escapes are consumed as opaque single elements (their internal validity is never checked, only their extent). Leading `]` right after `[`/`[^]` (a rare Oniguruma idiom for a class that contains a literal `]`) is not special-cased and is treated as an ordinary close bracket, matching RuboCop's own spec suite which does not exercise it either.
