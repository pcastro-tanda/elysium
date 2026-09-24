# Style/RedundantCondition

Checks for unnecessary conditional expressions.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

NOTE: Since the intention of the comment cannot be automatically determined,
autocorrection is not applied when a comment is used inside the conditional.

```ruby
# bad
a = b ? b : c

# good
a = b || c

# bad
if b
  b
else
  c
end

# good
b || c

# good
if b
  b
elsif cond
  c
end

# bad
a.nil? ? true : a

# good
a.nil? || a

# bad
if a.nil?
  true
else
  a
end

# good
a.nil? || a
```

With `AllowedMethods: ['nonzero?']` (the default), a predicate call in that
list is exempt from the "true branch is a bare `true`" check:

```ruby
# good
num.nonzero? ? true : false
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `nonzero?` |  | Predicate methods allowed as the condition when the true branch is a bare `true` literal. |

## Blind spots

Node equality (RuboCop's AST `==`, e.g. `condition == if_branch`) is
approximated by comparing the exact source text of the two spans; two
structurally-identical expressions written with different literal
formatting (extra parens, different whitespace) are treated as unequal,
matching real-world code but a narrower notion than RuboCop's true AST
comparison.

`node.parent&.send_type?` (used to decide whether the reconstructed `||`
form needs wrapping parens) is approximated from the parent's `NodeKind`
alone, so it does not distinguish a safe-navigation (`&.`) parent call from
a plain one -- both wrap, which is always safe to do.

`node.each_descendant.any? { contains_comments? }` (RuboCop's per-descendant,
per-line comment scan that blocks autocorrection) is approximated as "any
comment anywhere inside the `if`/`unless` node's byte range", a coarser
but strictly more conservative check: it only ever skips a fix RuboCop would
have applied, never the reverse.

A branch made of two or more `;`- or newline-separated statements that all
fit on a single source line (an implicit `begin` RuboCop can still call
`.source` on) is not reconstructed into the `||` form; the offense is
skipped entirely. Not covered by any known RuboCop spec.
