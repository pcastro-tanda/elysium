# Style/IfUnlessModifierOfIfUnless

Avoid modifier if/unless usage on conditionals.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

Checks for `if` and `unless` statements used as modifiers of other `if` or
`unless` statements.

```ruby
# bad
tired? ? 'stop' : 'go faster' if running?

# bad
if tired?
  "please stop"
else
  "keep going"
end if running?

# good
if running?
  tired? ? 'stop' : 'go faster'
end
```

## Options

This rule has no options.

## Blind spots

A modifier `if`/`unless` whose body is a nested modifier `if`/`unless` (e.g.
`a if b if c`) needs two fix-and-reparse rounds to reach RuboCop's final
correction, since the outer node's fix would otherwise collide with the
inner node's insertion at the same offset; the engine's fix loop already
reruns rules to convergence, so this only affects how many rounds it takes,
not the final output.
