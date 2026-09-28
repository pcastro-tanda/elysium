# Lint/PercentSymbolArray

Checks for unwanted commas and colons in %i/%I literals.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for colons and commas in `%i`, e.g. `%i(:foo, :bar)`

It is more likely that the additional characters are unintended (for
example, mistranslating an array of literals to percent string notation)
rather than meant to be part of the resulting symbols.

```ruby
# bad
%i(:foo, :bar)

# good
%i(foo bar)
```

## Options

This rule has no options.

## Blind spots

None recorded.
