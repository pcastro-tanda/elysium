# Style/InfiniteLoop

Use Kernel#loop for infinite loops. This cop is unsafe if the body may raise a `StopIteration` exception.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Use `Kernel#loop` for infinite loops.

@safety
  This cop is unsafe as the rule should not necessarily apply if the loop
  body might raise a `StopIteration` exception; contrary to other infinite
  loops, `Kernel#loop` silently rescues that and returns `nil`.

## Options

This rule has no options.

## Blind spots

`Layout/IndentationWidth`'s `Width` is read as a peer option (falling back to 2), matching upstream's own `Alignment#configured_indentation_width` cross-cop read.
