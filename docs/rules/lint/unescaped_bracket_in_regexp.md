# Lint/UnescapedBracketInRegexp

Checks for unescaped literal `]` in Regexp.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for unescaped literal `]` in Regexp.

```ruby
# bad
re = /[abc]]/

# good
re = /[abc]\]/
```

## Options

This rule has no options.

## Blind spots

None recorded.
