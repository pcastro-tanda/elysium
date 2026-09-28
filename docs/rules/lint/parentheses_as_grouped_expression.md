# Lint/ParenthesesAsGroupedExpression

Checks for method calls with a space before the opening parenthesis.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for space between the name of a called method and a left
parenthesis.

```ruby
# bad
do_something (foo)

# good
do_something(foo)
do_something (2 + 3) * 4
do_something (foo * bar).baz
```

## Options

This rule has no options.

## Blind spots

Upstream's `valid_context?`/`valid_first_argument?`/`chained_calls?` guard
against several `node.first_argument` shapes (a block, a call chain, an
operator keyword, a hash, a ternary, a parenthesized range) that can never
actually occur once `parenthesized_call?` has already proven the argument is
a real-parens wrapper node; see the module doc for why. Omitted as dead code,
not a behavior change.
