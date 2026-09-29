# Style/ParallelAssignment

Check for simple usages of parallel assignment. It will only warn when the number of variables matches on both sides of the assignment.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

This will only complain when the number of variables being assigned matched the number of assigning variables.

```ruby
# bad
a, b, c = 1, 2, 3
a, b, c = [1, 2, 3]

# good
one, two = *foo
a, b = foo
a, b = b, a

a = 1
b = 2
c = 3
```

## Options

This rule has no options.

## Blind spots

`accesses?`/`uses_var?`'s receiver comparisons use exact source-text equality (this codebase's established approximation for `Parser::AST::Node#==`, see `lint/self_assignment.rs`) rather than true structural equality: two differently-formatted-but-equal receiver expressions (extra parens, different whitespace) would be treated as distinct and so never reordered relative to each other, matching every fixture but not upstream's generic node equality in the abstract.
