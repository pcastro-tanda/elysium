# Style/DirectiveScope

Checks for directive scopes that can be expressed with the tighter `disable-next` form.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for directive scopes that can be expressed with the tighter
next-statement forms: a `disable`/`enable` pair, an
`enable`/`disable` pair, or a `push`/`pop` with signed arguments
wrapping exactly one statement. A statement-scoped directive cannot
drift as the surrounding code changes and needs no closing boundary.

@safety
  The autocorrection is unsafe because the suppression scope shrinks
  from the whole region to the statement: offenses of the suppressed
  cops located on the directive lines themselves resurface.

## Options

This rule has no options.

## Blind spots

Ranges are grouped by each directive's own literal reference (`all`, a bare
department word, or a slash-qualified cop path) rather than by real,
registry-expanded cop name -- see the module docs for why the exact-cop-list
match this cop already requires between a directive and its closing pair
rules out the only case that gap would otherwise bite.
