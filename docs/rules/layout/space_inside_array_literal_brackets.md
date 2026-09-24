# Layout/SpaceInsideArrayLiteralBrackets

Checks the spacing inside array literal brackets.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Checks that brackets used for array literals have or don't have
surrounding space depending on configuration.

Array pattern matching (`in [1, 2]`, `Const[1, 2]`) is handled the same way.

```ruby
# EnforcedStyle: no_space (default)
# The `no_space` style enforces that array literals have
# no surrounding space.

# bad
array = [ a, b, c, d ]
array = [ a, [ b, c ]]

# good
array = [a, b, c, d]
array = [a, [b, c]]
```

```ruby
# EnforcedStyle: space
# The `space` style enforces that array literals have
# surrounding space.

# bad
array = [a, b, c, d]
array = [ a, [ b, c ]]

# good
array = [ a, b, c, d ]
array = [ a, [ b, c ] ]
```

```ruby
# EnforcedStyle: compact
# The `compact` style normally requires a space inside
# array brackets, with the exception that successive left
# or right brackets are collapsed together in nested arrays.

# bad
array = [a, b, c, d]
array = [ a, [ b, c ] ]
array = [
  [ a ],
  [ b, c ]
]

# good
array = [ a, b, c, d ]
array = [ a, [ b, c ]]
array = [[ a ],
  [ b, c ]]
```

```ruby
# EnforcedStyleForEmptyBrackets: no_space (default)
# The `no_space` EnforcedStyleForEmptyBrackets style enforces that
# empty array brackets do not contain spaces.

# bad
foo = [ ]
bar = [     ]

# good
foo = []
bar = []
```

```ruby
# EnforcedStyleForEmptyBrackets: space
# The `space` EnforcedStyleForEmptyBrackets style enforces that
# empty array brackets contain exactly one space.

# bad
foo = []
bar = [    ]

# good
foo = [ ]
bar = [ ]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `no_space` | `space`, `no_space`, `compact` | Whether array literal brackets require, forbid, or collapse surrounding space. |
| EnforcedStyleForEmptyBrackets | `no_space` | `space`, `no_space` | Whether empty array literal brackets (`[]`) require or forbid a single interior space. |

## Blind spots

`multi_dimensional_array?`/`next_to_comment?`/`next_to_newline?` are
re-derived as raw byte scans over source text rather than RuboCop's token
stream (see the module doc for the equivalence argument). The one case this
does not model exactly: a line comment sitting directly between two
brackets whose own text happens to end in `[` or `]` (e.g. `] # ]`) would
be misread as bracket-adjacent by a naive scan; this implementation instead
stops at the comment's leading `#` (never a bracket), same as RuboCop's own
token-adjacency check, so no divergence is expected in practice.

Ruby's `\s` (whitespace) is modeled as ASCII space/tab/newline/CR/FF/VT;
no Unicode whitespace is treated as blank, matching MRI's own byte-based
lexer.

RuboCop's `autocorrect_with_disable_uncorrectable?` gate (the
`--disable-uncorrectable` CLI flag suppressing one side of a two-sided
offense) has no equivalent here: this engine does not model that CLI mode,
so both sides of a two-sided offense are always reported.
