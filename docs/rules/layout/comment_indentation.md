# Layout/CommentIndentation

Checks the indentation of comments.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

An own-line comment (one with only blanks before it on its line) is expected
to be indented like the code that follows it -- specifically, like the first
non-blank line after it, one extra level deeper when that line is a closing
`end`/`)`/`}`/`]` (the comment introduces the block being closed, not the
close itself).

```ruby
# bad
    # comment here
def method_name
end

# good
# comment here
def method_name
end
```

A comment directly before `else`/`elsif`/`when`/`in`/`rescue`/`ensure` may be
aligned with either that keyword or the statement above it:

```ruby
# good
if a
  b
# this is accepted
elsif aa
  # so is this
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowForAlignment | false |  | Allow comments to have extra indentation if that aligns them with a trailing comment on the nearest preceding non-own-line comment. |

## Blind spots

`Layout/IndentationWidth`'s `Width` and `Layout/AccessModifierIndentation`'s
`EnforcedStyle` are read as peer options, matching upstream's own
cross-cop reads.
