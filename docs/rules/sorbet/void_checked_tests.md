# Sorbet/VoidCheckedTests

Forbid `.void.checked(:tests)`

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Disallows the usage of `.void.checked(:tests)`.

```ruby
# bad
sig { void.checked(:tests) }

# good
sig { void }
sig { returns(T.anything).checked(:tests) }
sig { void.checked(:never) }
```

## Options

This rule has no options.

## Blind spots

None recorded.
