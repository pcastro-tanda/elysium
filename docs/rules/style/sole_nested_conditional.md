# Style/SoleNestedConditional

Finds sole nested conditional nodes which can be merged into outer conditional node.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

If the branch of a conditional consists solely of a conditional node, its
conditions can be combined with the conditions of the outer branch. This
helps to keep the nesting level from getting too deep.

```ruby
# bad
if condition_a
  if condition_b
    do_something
  end
end

# bad
if condition_b
  do_something
end if condition_a

# good
if condition_a && condition_b
  do_something
end
```

With `AllowModifier: false` (the default):

```ruby
# bad
if condition_a
  do_something if condition_b
end
```

With `AllowModifier: true`:

```ruby
# good
if condition_a
  do_something if condition_b
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowModifier | false |  | Allow modifier-form `if`/`unless` to be nested without merging. |

## Blind spots

`use_variable_assignment_in_condition?` (the guard against merging an outer variable
assignment into a nested modifier that reads that same variable) only recognizes plain
local-variable targets (`var = ...`, `var &&= ...`, `var ||= ...`, `var op= ...`);
instance/class/global-variable and constant assignments in the outer condition are not
tracked, so an extremely rare `if @x = foo
  do_something if @x
end` would be flagged
where real RuboCop stays silent.
Assignment detection for a plain (single `=`) attribute or index writer (`obj.attr =
val`, `arr[i] = val`) is approximated via Prism's `equal_loc` presence on a `CallNode`
rather than RuboCop's exact `setter_method?` check.
`self.autocorrect_incompatible_with` (avoiding a double-correction clash with
`Style/NegatedIf`/`Style/NegatedUnless` when both cops run together) is not
implemented; each cop computes its fix independently.
