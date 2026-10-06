# Sorbet/AllowIncompatibleOverride

Disallows using `.override(allow_incompatible: true)`.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Disallows using `.override(allow_incompatible: true)`.

```ruby
# bad
sig.override(allow_incompatible: true)

# good
sig.override
```

## Options

This rule has no options.

## Blind spots

None recorded.
