# Lint/AmbiguousOperator

Checks for ambiguous operators in the first argument of a method invocation without parentheses.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

```ruby
# bad

# The `*` is interpreted as a splat operator but it could possibly be
# a `*` method invocation (i.e. `do_something.*(some_array)`).
do_something *some_array

# good

# With parentheses, there's no ambiguity.
do_something(*some_array)
```

## Options

This rule has no options.

## Blind spots

Relies entirely on Prism's own parser warnings for ambiguity detection rather than reimplementing RuboCop's whitequark-based diagnostic logic; any case where Prism's parser disagrees with whitequark's about what counts as an ambiguous prefix is not reproduced. `self.autocorrect_incompatible_with` (`Naming::BlockForwarding`) is not ported: this port has no cross-rule autocorrect-conflict mechanism.
