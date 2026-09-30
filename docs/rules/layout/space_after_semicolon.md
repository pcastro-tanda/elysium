# Layout/SpaceAfterSemicolon

Use spaces after semicolons.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
x = 1;y = 2

# good
x = 1; y = 2
```

## Options

This rule has no options.

## Blind spots

Reproduces RuboCop's token-stream-based mixin by scanning raw bytes for `;`
outside every opaque span (see `space_punctuation`'s module docs); this
reaches every case the cop's own logic reaches except a semicolon directly
followed by the very end of a multi-byte percent-literal delimiter or other
lexer-only token shape that has no raw-byte tell of its own, none of which
occur adjacent to a semicolon in valid Ruby.
