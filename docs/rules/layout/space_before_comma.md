# Layout/SpaceBeforeComma

No spaces before commas.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
[1 , 2 , 3]
a(1 , 2)
each { |a , b| }

# good
[1, 2, 3]
a(1, 2)
each { |a, b| }
```

## Options

This rule has no options.

## Blind spots

Reproduces RuboCop's token-stream-based mixin by scanning raw bytes for `,`
outside every opaque span (see `space_punctuation`'s module docs); this
reaches every case the cop's own logic reaches.
