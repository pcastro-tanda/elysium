# Style/AndOr

Use &&/|| instead of and/or.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for uses of `and` and `or`, and suggests using `&&` and
`||` instead. It can be configured to check only in conditions or in
all contexts.

```ruby
# EnforcedStyle: conditionals (default)

# bad
if foo and bar
end

# good
foo.save && return

# good
foo.save and return

# good
if foo && bar
end
```

```ruby
# EnforcedStyle: always

# bad
foo.save and return

# bad
if foo and bar
end

# good
foo.save && return

# good
if foo && bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `conditionals` | `conditionals`, `always` | Whether `and`/`or` are banned only in conditionals (`conditionals`) or completely (`always`). |

## Blind spots

Autocorrection is unsafe because there is a different operator precedence between logical
operators (`&&`/`||`) and semantic operators (`and`/`or`), and that might change behaviour --
inherited directly from upstream's own `@safety` note. RuboCop-AST's `parenthesized_call?` (used
to avoid re-wrapping an already-parenthesized `return`/`next`/`break` operand) is approximated as
always false for those three: Prism represents `return(x)` by wrapping `x` in its own
`ParenthesesNode` rather than giving the `return` node a paren location of its own, unlike
whitequark. `yield(x)`'s own `lparen_loc` is checked directly, and every other operand shape
(comparison calls, bare commands, `not`) already carries an equivalent location this port checks
instead.
