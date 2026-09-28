# Style/BlockComments

Do not use block comments.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Looks for uses of block comments (`=begin`...`=end`).

```ruby
# bad
=begin
Multiple lines
of comments...
=end

# good
# Multiple lines
# of comments...
```

## Options

This rule has no options.

## Blind spots

None recorded.
