# Sorbet/KeywordArgumentOrdering

Enforces a compatible keyword arguments with Sorbet.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for the ordering of keyword arguments required by sorbet-runtime. All keyword arguments must be at the end of the parameters list, and all keyword arguments with a default value must be after those without default values.

```ruby
# bad
sig { params(a: Integer, b: String).void }
def foo(a: 1, b:); end

# good
sig { params(b: String, a: Integer).void }
def foo(b:, a: 1); end
```

## Options

This rule has no options.

## Blind spots

None recorded.
