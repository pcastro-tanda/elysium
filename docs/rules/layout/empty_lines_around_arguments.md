# Layout/EmptyLinesAroundArguments

Keeps track of empty lines around method arguments.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

A blank line right after a call's opening parenthesis, between two
arguments, or right before the closing parenthesis is noise:

```ruby
# bad
do_something(
  foo

)

# good
do_something(
  foo
)
```

The fix removes each such blank line.

## Options

This rule has no options.

## Blind spots

None recorded.
