# Style/MultilineIfModifier

Checks for uses of if/unless modifiers with multiple-lines bodies.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of if/unless modifiers with multiple-lines bodies.

```ruby
# bad
{
  result: 'this should not happen'
} unless cond

# good
{ result: 'ok' } if cond
```

## Options

This rule has no options.

## Blind spots

Reads `Layout/IndentationWidth`'s `Width` as a peer option (`Alignment#configured_indentation_width`).
