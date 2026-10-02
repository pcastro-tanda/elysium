# Style/OperatorMethodCall

Checks for redundant dot before operator method call.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

The target operator methods are `|`, `^`, `&`, `<=>`, `==`, `===`, `=~`,
`>`, `>=`, `<`, `<=`, `<<`, `>>`, `+`, `-`, `*`, `/`, `%`, `**`, `~`, `!`,
`!=`, and `!~`.

```ruby
# bad
foo.+ bar
foo.& bar

# good
foo + bar
foo & bar
```

## Options

This rule has no options.

## Blind spots

None recorded.
