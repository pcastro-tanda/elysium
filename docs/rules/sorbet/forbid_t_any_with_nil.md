# Sorbet/ForbidTAnyWithNil

Forbid usage of T.any(NilClass).

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Detect and autocorrect `T.any(..., NilClass, ...)` to `T.nilable(...)`.

```ruby
# bad
T.any(String, NilClass)
T.any(NilClass, String)
T.any(NilClass, Symbol, String)

# good
T.nilable(String)
T.nilable(String)
T.nilable(T.any(Symbol, String))
```

## Options

This rule has no options.

## Blind spots

None recorded.
