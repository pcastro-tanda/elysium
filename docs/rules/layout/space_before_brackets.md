# Layout/SpaceBeforeBrackets

Checks for receiver with a space before the opening brackets.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

```ruby
# bad
collection [index_or_key]

# good
collection[index_or_key]
```

## Options

This rule has no options.

## Blind spots

None recorded.
