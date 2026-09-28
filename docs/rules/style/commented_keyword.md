# Style/CommentedKeyword

Do not place comments on the same line as certain keywords.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for comments put on the same line as some keywords. These keywords are: `class`, `module`, `def`, `begin`, `end`.

Note that some comments (`:nodoc:`, `:yields:`, `rubocop:disable` and `rubocop:todo`), RBS::Inline annotation, and Steep annotation (`steep:ignore`) are allowed.

Autocorrection removes comments from `end` keyword and keeps comments for `class`, `module`, `def` and `begin` above the keyword.

```ruby
# bad
if condition
  statement
end # end if

# bad
class X # comment
  statement
end

# bad
def x; end # comment

# good
if condition
  statement
end

# good
class X # :nodoc:
  y
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
