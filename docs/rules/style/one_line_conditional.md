# Style/OneLineConditional

Favor the ternary operator (?:) or multi-line constructs over single-line if/then/else/end constructs.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of `if/then/else/end` constructs on a single line.
A ternary operator (`?:`) or multi-line `if` is more readable.
`AlwaysCorrectToMultiline` config option can be set to `true` to autocorrect all offenses to
multi-line constructs. When `AlwaysCorrectToMultiline` is `false` (default case) the
autocorrect will first try converting them to ternary operators.

```ruby
# bad
if foo then bar else baz end

# bad
unless foo then baz else bar end

# good
foo ? bar : baz

# good
bar if foo

# good
if foo then bar end

# good
if foo
  bar
else
  baz
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AlwaysCorrectToMultiline | false |  | When `true`, autocorrects every offense to a multi-line construct instead of first trying a ternary operator. |

## Blind spots

`node.parent.operator_keyword?`/`node.parent.send_type? && node.parent.operator_method?` (used to decide whether the ternary replacement needs an extra pair of wrapping parens) is approximated by reading the literal operator token back from source text between the relevant ancestor's span start and this node's own span start, since `Context::ancestors` carries only kind and span, not the full parent node. This matches every shape a real operator call can take here (including the `~`/`!` "unary-looking infix" reparse -- see the module doc) but would misidentify an explicitly parenthesized operator call (`a.+(if cond then x else y end)`) as not needing a wrap; no known spec exercises that spelling.
