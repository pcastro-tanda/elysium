# Layout/SpaceBeforeFirstArg

Checks that exactly one space is used between a method name and the first argument for method calls without parentheses.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
something  x
something   y, z
something'hello'

# good
something x
something y, z
something 'hello'
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowForAlignment | true |  | Allow extra spacing that lines up the first argument with the previous or next line. |

## Blind spots

`AllowForAlignment`'s alignment search only matches RuboCop's own `aligned_words?` half of
`aligned_token?` (an exact-text or space-then-non-space column match on another line);
`aligned_equals_operator?`'s equals-sign fallback is not reproduced, since it requires the
checked range's own source text to end in `=`, which a method call's first argument
practically never does. Column comparisons index by byte offset within a line, i.e. assume
one byte per character; a line with multi-byte UTF-8 content before the compared column can
misalign the comparison.
