# Lint/PercentStringArray

Checks for unwanted commas and quotes in %w/%W literals.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for quotes and commas in `%w`, e.g. `%w('foo', "bar")`

It is more likely that the additional characters are unintended (for
example, mistranslating an array of literals to percent string notation)
rather than meant to be part of the resulting strings.

```ruby
# bad
%w('foo', "bar")

# good
%w(foo bar)
```

## Options

This rule has no options.

## Blind spots

None recorded.
