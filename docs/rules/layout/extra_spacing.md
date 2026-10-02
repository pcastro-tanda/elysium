# Layout/ExtraSpacing

Checks for extra/unnecessary whitespace.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# good if AllowForAlignment is true
name      = "RuboCop"
# Some comment and an empty line

website  += "/rubocop/rubocop" unless cond
puts        "rubocop"          if     debug

# bad for any configuration
set_app("RuboCop")
website  = "https://github.com/rubocop/rubocop"

# good only if AllowBeforeTrailingComments is true
object.method(arg)  # this is a comment

# good even if AllowBeforeTrailingComments is false or not set
object.method(arg) # this is a comment

# good with either AllowBeforeTrailingComments or AllowForAlignment
object.method(arg)         # this is a comment
another_object.method(arg) # this is another comment
some_object.method(arg)    # this is some comment
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowForAlignment | true |  | Allow extra spacing that lines up code across adjacent lines. |
| AllowBeforeTrailingComments | false |  | Allow extra spacing before a trailing end-of-line comment. |
| ForceEqualSignAlignment | false |  | Force the alignment of `=` in assignments on consecutive lines. |

## Blind spots

There is no real token stream to work from (see the module docs for the
approximation this file builds instead), which has these consequences:

- `spacing_varies?`'s `(column, type)` grouping (1.91.0) relies on
  [`TokKind`], a coarse lexical classification (word/number/literal/one
  operator byte) rather than RuboCop's real, fine-grained lexer token
  types; two genuinely different real token types that happen to share a
  [`TokKind`] at the same column could be grouped together (or the
  reverse) when they wouldn't be upstream, which can tip `spacing_varies?`
  either way.
- `interrupting_operator_lines` (1.91.0, `ForceEqualSignAlignment`'s
  block-boundary detection) finds a bare `<<` append operator lexically,
  telling it apart from a bare heredoc opener (`<<HEREDOC`, no `~`/`-`) by
  requiring an expression-like byte immediately before it; a heredoc
  opener directly preceded by such a byte (unusual, but not impossible)
  would be misdetected as an interrupting append operator.
- Column/token comparisons index by byte offset within a line, i.e. assume
  one byte per character; a line with multi-byte UTF-8 content before the
  compared column can misalign the comparison (offense spans themselves
  remain exact byte spans, unaffected).
- `token_extent`'s lexical tokenizer (identifiers, `.`/`..`/`...`, `::`,
  and a greedy run of RuboCop's operator-alphabet characters) is not a
  full Ruby lexer; an unusual unspaced operator sequence could glom more
  characters into `AllowForAlignment`'s exact-text fallback than a real
  token would, which can only make that fallback harder to satisfy.
