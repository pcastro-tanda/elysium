# Sorbet/CheckedTrueInSignature

Disallows the usage of `checked(true)` in signatures.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Disallows the usage of `checked(true)`.

```ruby
# bad
sig { void.checked(true) }

# good
sig { void }
```

## Options

This rule has no options.

## Blind spots

None recorded.
