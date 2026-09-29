# Style/IfInsideElse

Finds if nodes inside else, which can be converted to elsif.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

If the `else` branch of a conditional consists solely of an `if` node, it can be combined with the `else` to become an `elsif`. This helps to keep the nesting level from getting too deep.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowIfModifier | false |  | Allow a modifier-form `if` (`foo if bar`) as the sole content of an `else`. |

## Blind spots

`range_with_comments`'s real comment association (`ast_with_comments`) is
reconstructed as: a contiguous run of comment-only lines directly above the
if-branch (no blank-line gap), plus a single trailing same-line comment
after it. A comment separated from the branch by a blank line, or floating
inside a multi-statement branch, is not folded in and is instead silently
dropped by the surrounding whole-line deletion -- unobserved in the corpus
so far.
