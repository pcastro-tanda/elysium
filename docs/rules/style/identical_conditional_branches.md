# Style/IdenticalConditionalBranches

Checks that conditional statements do not have an identical line at the end of each branch, which can validly be moved out of the conditional.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for identical expressions at the beginning or end of each branch of
a conditional expression. Such expressions should normally be placed
outside the conditional expression - before or after it.

```ruby
# bad
if condition
  do_x
  do_z
else
  do_y
  do_z
end

# good
if condition
  do_x
else
  do_y
end
do_z

# bad
if condition
  do_z
  do_x
else
  do_z
  do_y
end

# good
do_z
if condition
  do_x
else
  do_y
end

# bad
case foo
when 1
  do_x
when 2
  do_x
else
  do_x
end

# good
case foo
when 1
  do_x
  do_y
when 2
  # nothing
else
  do_x
  do_z
end
```

## Options

This rule has no options.

## Blind spots

Autocorrection is unsafe: it can reorder method invocations across a
condition that depends on global state. The condition-matches-target guard
that blocks hoisting `x = do_something` above `if x.condition` (or a
`send`/`csend` setter target) is ported faithfully; the equivalent guard
for every other assignment shape (shorthand `op_asgn`/`or_asgn`/`and_asgn`,
`casgn`, `masgn`, a `csend` setter target) is upstream-inert (it compares a
plain identifier against a `Node#inspect`-style string that can never
match) and is reproduced here by simply never matching those shapes,
rather than replicating the unreachable comparison. `last_child_of_parent?`
is exact when the conditional's parent is a `StatementsNode` (its usual
position) or has no parent; for a non-`StatementsNode`, non-nil parent
(e.g. a hash value, an array element, a call argument) it defaults to
`true`, matching every parent shape exercised by the corpus but not
verified against upstream's own `child_nodes.last == node` check for the
untested shapes (e.g. the conditional as a non-last call argument).
