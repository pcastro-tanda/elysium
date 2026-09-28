# Layout/EmptyComment

Checks empty comment.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

An empty `#` line carries no information and is usually leftover noise.

```ruby
# bad

#
class Foo
end

# good

#
# Description of `Foo` class.
#
class Foo
end
```

By default, a comment made entirely of `#` characters (a "border", e.g.
`#####`) and a bare `#` immediately adjacent to a real comment (a
"margin", used to frame it) are both left alone; `AllowBorderComment` and
`AllowMarginComment` turn either of those off.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowBorderComment | true |  | Allow comments that consist only of `#` characters (borders). |
| AllowMarginComment | true |  | Allow a bare `#` line adjacent to a non-empty comment (a margin). |

## Blind spots

Whether a comment trails code on its own line is approximated with
[`Context::begins_its_line`] rather than a real previous-token lookup;
this matches for every comment shape RuboCop itself considers (a comment
is always the last token on its line).
