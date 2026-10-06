# Sorbet/RuntimeOnFailureDependsOnChecked

Ensures that `on_failure` is called after `checked` in signatures.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks that `on_failure` is not used without `checked(:tests)` or `checked(:always)`.

```ruby
# bad
sig { params(x: Integer).returns(Integer).on_failure(:raise) }

# good
sig { params(x: Integer).returns(Integer).checked(:always).on_failure(:raise) }
```

## Options

This rule has no options.

## Blind spots

None recorded.
