# Layout/MultilineOperationIndentation

Checks indentation of binary operations that span more than one line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks the indentation of the right hand side operand in binary operations
that span more than one line.

The `aligned` style checks that operators are aligned if they are part of
an `if` or `while` condition, an explicit `return` statement, etc. In other
contexts, the second operand should be indented regardless of enforced
style.

In both styles, operators should be aligned when an assignment begins on
the next line.

```ruby
# EnforcedStyle: aligned (default)

# bad
if a +
    b
  something &&
  something_else
end

# good
if a +
   b
  something &&
    something_else
end
```

```ruby
# EnforcedStyle: indented

# bad
if a +
   b
  something &&
  something_else
end

# good
if a +
    b
  something &&
    something_else
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `aligned` | `aligned`, `indented` | Aligns the operands in keyword/assignment contexts (`aligned`) or always indents the right operand one width (`indented`). |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width. Only accepted when `EnforcedStyle` is `indented`. |

## Blind spots

Autocorrection shifts the offending range exactly as upstream's
`AlignmentCorrector` does when handed a bare range: without the heredoc
taboo ranges it computes for a node, since upstream passes the offense
range, not the node.
