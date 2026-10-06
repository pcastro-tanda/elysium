# Sorbet/ForbidTUnsafe

Forbid usage of T.unsafe.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Disallows using `T.unsafe` anywhere.
Set `AutocorrectToRBS: true` to replace supported calls with RBS inline comments.

```ruby
# bad
T.unsafe(foo)

# good
foo #: as untyped
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AutocorrectToRBS | false |  | Replace supported `T.unsafe` calls with RBS inline comments. |

## Blind spots

None recorded.
