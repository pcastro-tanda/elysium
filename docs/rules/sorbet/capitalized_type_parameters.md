# Sorbet/CapitalizedTypeParameters

Ensures that type parameters are capitalized.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Ensure type parameters used in generic methods are always capitalized.

```ruby
# bad
sig { type_parameters(:x).params(a: T.type_parameter(:x)).void }
def foo(a); end

# good
sig { type_parameters(:X).params(a: T.type_parameter(:X)).void }
def foo(a: 1); end
```

## Options

This rule has no options.

## Blind spots

Symbol#inspect of the corrected value is approximated: names made of alphanumerics, `_` and non-ASCII characters (with an optional trailing `?`, `!` or `=`) print bare, anything else is double-quoted.
