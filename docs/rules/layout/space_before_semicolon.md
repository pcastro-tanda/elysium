# Layout/SpaceBeforeSemicolon

No spaces before semicolons.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
x = 1 ; y = 2

# good
x = 1; y = 2
```

## Options

This rule has no options.

## Blind spots

Reproduces RuboCop's token-stream-based mixin by scanning raw bytes for `;`
outside every opaque span (see `space_punctuation`'s module docs); this
reaches every case the cop's own logic reaches.
