# Layout/CaseIndentation

Checks how the `when` and `in` clauses of a `case` expression are indented.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks how the `when` and `in` clauses of a `case` expression are indented in
relation to its `case` or `end` keyword. It will register a separate offense
for each misaligned `when` and `in`.

If `Layout/EndAlignment` is set to keyword style (default), `case` and `end`
should always be aligned to the same depth, and therefore `when` should
always be aligned to both -- regardless of configuration.

With `EnforcedStyle: case` (the default), `when`/`in` is measured against the
`case` keyword's own column; with `EnforcedStyle: end`, against the `end`
keyword's column (only meaningful when `Layout/EndAlignment`'s
`EnforcedStyleAlignWith` is set to something other than `keyword`).
`IndentOneStep` shifts the expected column one further step (this cop's
`Layout/IndentationWidth`, or its own `IndentationWidth` override) past the
base.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `case` | `case`, `end` | Whether `when`/`in` is measured against `case` or `end`. |
| IndentOneStep | false |  | Whether `when`/`in` should be indented one step further than the base, rather than the same depth. |
| IndentationWidth | `nil` |  | Overrides `Layout/IndentationWidth`'s configured width for `IndentOneStep`'s extra step; falls back to it, else 2. |

## Blind spots

None recorded.
