# Layout/ClosingParenthesisIndentation

Checks the indentation of hanging closing parentheses.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the indentation of hanging closing parentheses in
method calls, method definitions, and grouped expressions. A hanging
closing parenthesis means `)` preceded by a line break.

```ruby
# bad
some_method(
  a,
  b
  )

# good: when first param is on a new line, right paren is *always*
#       outdented by IndentationWidth
some_method(
  a,
  b
)

# good: when all other params are also on the same line, outdent
#       right paren by IndentationWidth
some_method(a, b, c
           )

# good: when all other params are on multiple lines, but are lined
#       up, align right paren with left paren
some_method(a,
            b,
            c
           )

# good: when other params are not lined up on multiple lines, outdent
#       right paren by IndentationWidth
some_method(a,
  x: 1,
  y: 2
)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width for the outdented-closing-paren column; falls back to it, else 2. |

## Blind spots

None recorded.
