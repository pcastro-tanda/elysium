# Style/ConditionalAssignment

Use the return value of `if` and `case` statements for assignment to a variable and variable comparison instead of assigning that variable inside of each branch.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for `if` and `case` statements where each branch is used for both the
assignment and comparison of the same variable when using the return of the
condition can be used instead.

```ruby
# EnforcedStyle: assign_to_condition (default)
# bad
if foo
  bar = 1
else
  bar = 2
end

# good
bar = if foo
        1
      else
        2
      end

# EnforcedStyle: assign_inside_condition
# bad
bar = if foo
        1
      else
        2
      end

# good
if foo
  bar = 1
else
  bar = 2
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `assign_to_condition` | `assign_to_condition`, `assign_inside_condition` | Whether to assign inside or outside of each conditional branch. |
| SingleLineConditionsOnly | true |  | Only register an offense when every branch is a single statement (assign_to_condition) or when no branch is multi-statement (assign_inside_condition). |
| IncludeTernaryExpressions | true |  | Whether ternary expressions should be considered by this cop. |

## Blind spots

`assign_to_condition`'s `lhs`'s `assignment_node.source` for an `op_asgn`/
`and_asgn`/`or_asgn` target is read off the target's own span/`name_loc`
rather than reconstructed textually; the two differ only when the original
source itself has unusual internal spacing inside a namespaced constant
path or an index's argument list, which no fixture exercises.
`assignment_rhs_exist?`'s `mlhs`/`resbody` guard is not needed: those
shapes never reach a Prism write-node kind this cop subscribes to in the
first place (a `for`-loop index is a bare target node, and a `rescue =>`
capture is too), so no offense is ever considered for them.
`safe_to_correct?` is ported as a real reparse of the corrected buffer, but
the `Parser::Source::TreeRewriter` clobbering errors it also rescues are
approximated by `merge_edits`: crossing removals merge into their union and
an insertion strictly inside a removal aborts, which is the only clobbering
shape either corrector can produce.
