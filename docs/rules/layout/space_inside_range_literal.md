# Layout/SpaceInsideRangeLiteral

Checks for spaces inside range literals.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
1 .. 3

# good
1..3

# bad
'a' .. 'z'

# good
'a'..'z'
```

## Options

This rule has no options.

## Blind spots

None recorded.
