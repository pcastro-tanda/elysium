# Sorbet/SignatureBuildOrder

Enforces the order of parts in a signature.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for the correct order of `sig` builder methods.

```ruby
# bad
sig { void.abstract }

# good
sig { abstract.void }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Order | `final`, `abstract`, `implementation`, `override`, `overridable`, `type_parameters`, `params`, `bind`, `returns`, `void`, `soft`, `checked`, `on_failure` |  | The order in which to enforce the builder methods are called. |

## Blind spots

None recorded.
