# Layout/SpaceInsideReferenceBrackets

Checks the spacing inside referential brackets.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that reference brackets have or don't have surrounding space
depending on configuration.

```ruby
# EnforcedStyle: no_space (default)
# The `no_space` style enforces that reference brackets have
# no surrounding space.

# bad
hash[ :key ]
array[ index ]

# good
hash[:key]
array[index]
```

```ruby
# EnforcedStyle: space
# The `space` style enforces that reference brackets have
# surrounding space.

# bad
hash[:key]
array[index]

# good
hash[ :key ]
array[ index ]
```

```ruby
# EnforcedStyleForEmptyBrackets: no_space (default)
# The `no_space` EnforcedStyleForEmptyBrackets style enforces that
# empty reference brackets do not contain spaces.

# bad
foo[ ]
foo[     ]
foo[
]

# good
foo[]
```

```ruby
# EnforcedStyleForEmptyBrackets: space
# The `space` EnforcedStyleForEmptyBrackets style enforces that
# empty reference brackets contain exactly one space.

# bad
foo[]
foo[    ]
foo[
]

# good
foo[ ]
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `no_space` | `space`, `no_space` | Whether reference brackets require or forbid surrounding space. |
| EnforcedStyleForEmptyBrackets | `no_space` | `space`, `no_space` | Whether empty reference brackets (`[]`) require or forbid a single interior space. |

## Blind spots

`on_send` only ever fires for `[]`/`[]=` method calls, never a method
*definition* (`def Vector.[](*array)`, a distinct node kind) and never a
safe-navigated call (`a&.[](1)`, which RuboCop's own `on_send` is never
dispatched to either since this cop defines no `on_csend`); both are
excluded here the same way.

Ruby's `\s` (whitespace) is modeled as ASCII space/tab/newline/CR/FF/VT;
no Unicode whitespace is treated as blank, matching MRI's own byte-based
lexer.

RuboCop's `autocorrect_with_disable_uncorrectable?` gate (the
`--disable-uncorrectable` CLI flag suppressing one side of a two-sided
offense) has no equivalent here: this engine does not model that CLI mode,
so both sides of a two-sided offense are always reported.
