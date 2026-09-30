# Layout/MultilineMethodCallBraceLayout

Checks that the closing brace in a method call is either on the same line as the last method argument, or a new line.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that the closing brace in a method call is either on the same line
as the last method argument, or a new line.

When using the `symmetrical` (default) style:

If a method call's opening brace is on the same line as the first argument
of the call, then the closing brace should be on the same line as the last
argument of the call.

If a method call's opening brace is on the line above the first argument of
the call, then the closing brace should be on the line below the last
argument of the call.

When using the `new_line` style, the closing brace of a multi-line method
call must be on the line after the last argument of the call.

When using the `same_line` style, the closing brace of a multi-line method
call must be on the same line as the last argument of the call.

```ruby
# EnforcedStyle: symmetrical (default)

# bad
foo(a,
  b
)

# bad
foo(
  a,
  b)

# good
foo(a,
  b)

# good
foo(
  a,
  b
)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `symmetrical` | `symmetrical`, `new_line`, `same_line` | Whether the closing brace mirrors the opening brace's own line (`symmetrical`), always sits on the line after the last argument (`new_line`), or always sits on the same line as the last argument (`same_line`). |

## Blind spots

If the closing delimiter's own physical line also holds a trailing chained
`.method`/`&.method` call *and* the last argument's own line carries a
trailing comment, both `content_if_comment_present`'s whole-line capture and
`use_heredoc_argument_method_chain?`'s chain relocation would need to
cooperate on the same stretch of text; RuboCop's own corrector has this same
gap (its two `TreeRewriter` actions are simply queued independently), and no
such case is reachable from this cop's own preconditions (the chain move
requires the call's own *first* argument to be a heredoc, and RuboCop's own
heredoc-safety guard already forbids autocorrecting a literal whose last
child's line carries a trailing comment together with a heredoc argument in
the same call).
