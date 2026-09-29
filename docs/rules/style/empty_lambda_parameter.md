# Style/EmptyLambdaParameter

Omit parens for empty lambda parameters.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for parentheses for empty lambda parameters. Parentheses for empty lambda parameters do not cause syntax errors, but they are redundant.

# bad
-> () { do_something }

# good
-> { do_something }

# good
-> (arg) { do_something(arg) }

## Options

This rule has no options.

## Blind spots

None recorded.
