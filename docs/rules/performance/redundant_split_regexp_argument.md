# Performance/RedundantSplitRegexpArgument

Identifies places where `split` argument can be replaced from a deterministic regexp to a string.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `split` argument can be replaced from a deterministic regexp to a string.

```ruby
# bad
'a,b,c'.split(/,/)

# good
'a,b,c'.split(',')
```

## Options

This rule has no options.

## Blind spots

None recorded.
