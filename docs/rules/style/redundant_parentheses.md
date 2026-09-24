# Style/RedundantParentheses

Checks for parentheses that seem not to serve any purpose.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Checks for redundant parentheses that don't change the meaning of the
expression -- around a bare variable, constant, or literal, around an
assignment, logical/comparison expression, keyword, or method call/unary
operation that doesn't need them to parse.

```ruby
# bad
(x) if ((y.z).nil?)

# good
x if y.z.nil?
```

## Options

This rule has no options.

## Blind spots

One-line pattern matching (`in`/`=>`) is only handled for the top-level `MatchPredicateNode`/`MatchRequiredNode` content case, not a full ancestor-walk guard for nested cases.
