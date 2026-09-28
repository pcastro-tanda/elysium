# Layout/SpaceInsideParens

Checks for spaces inside ordinary round parentheses.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# EnforcedStyle: no_space (default)
# The `no_space` style enforces that parentheses do not have spaces.

# bad
f( 3)
g = (a + 3 )
f( )

# good
f(3)
g = (a + 3)
f()
```

```ruby
# EnforcedStyle: space
# The `space` style enforces that parentheses have a space at the
# beginning and end.
# Note: Empty parentheses should not have spaces.

# bad
f(3)
g = (a + 3)
y( )

# good
f( 3 )
g = ( a + 3 )
y()
```

```ruby
# EnforcedStyle: compact
# The `compact` style enforces that parentheses have a space at the
# beginning with the exception that successive parentheses are allowed.
# Note: Empty parentheses should not have spaces.

# bad
f(3)
g = (a + 3)
y( )
g( f( x ) )
g( f( x( 3 ) ), 5 )
g( ( ( 3 + 5 ) * f) ** x, 5 )

# good
f( 3 )
g = ( a + 3 )
y()
g( f( x ))
g( f( x( 3 )), 5 )
g((( 3 + 5 ) * f ) ** x, 5 )
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `no_space` | `space`, `no_space`, `compact` | Whether parentheses require, forbid, or (for `compact`) selectively collapse surrounding space. |

## Blind spots

A `%`-literal whose closing delimiter is a lone `)` (`%(...)`, `%w(...)`,
`%i(...)`, `%r(...)`, `%q(...)`/`%Q(...)`, `%s(...)`, `%x(...)`) is not a
real paren token upstream (the whole literal, delimiters included, is one
lexer token), but if it sits immediately -- whitespace only, nothing else
between -- before a real close paren inside a checked node's own content
(e.g. `f(%(a) )`), this rule's `compact`-style consecutive-close-paren
check reads that literal's trailing `)` byte as a neighbouring real paren
and would collapse the single space between them. This mirrors a known gap
in `SpaceInsideArrayLiteralBrackets`'s own token-stream re-derivation and is
expected to be exercised only by deliberately adversarial input.
