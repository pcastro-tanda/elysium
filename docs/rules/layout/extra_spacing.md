# Layout/ExtraSpacing

Checks for extra/unnecessary whitespace.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | nursery |

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

- `AllowForAlignment`'s alignment search only matches RuboCop's own
  `ASSIGNMENT_OR_COMPARISON_TOKENS` for the equals-sign fallback with plain
  `=` and the compound assignment operators (`+=`, `-=`, ..., `**=`,
  `<<=`, `>>=`, `||=`, `&&=`); comparison operators (`==`, `===`, `!=`,
  `<=`, `>=`) and the bare `<<` append operator are not tracked as
  alignment landmarks. This can only under-recognize an alignment RuboCop
  would allow (an over-reporting risk), not the reverse; no fixture in
  this port exercises it.
- The mixin's second `aligned_with_any_line_range?` pass (retrying with a
  `base_indentation` filter after an unfiltered scan already failed) is
  not implemented: for this cop's call sites the filtered scan is always a
  strict subset of lines the unfiltered scan already visited with the same
  predicate, so it can never change the result.
- Column/token comparisons index by byte offset within a line, i.e. assume
  one byte per character; a line with multi-byte UTF-8 content before the
  compared column can misalign the comparison (offense spans themselves
  remain exact byte spans, unaffected).
- `token_extent`'s lexical tokenizer (identifiers, `.`/`..`/`...`, `::`,
  and a greedy run of RuboCop's operator-alphabet characters) is not a
  full Ruby lexer; an unusual unspaced operator sequence could glom more
  characters into `AllowForAlignment`'s exact-text fallback than a real
  token would, which can only make that fallback harder to satisfy.
