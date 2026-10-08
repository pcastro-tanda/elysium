# Sorbet/ForbidTBindInAssignment

Forbid assigning the result of T.bind.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Disallows assigning the result of `T.bind`.

`T.bind` changes the type of its first argument and returns that argument. Assigning its result can therefore unintentionally change the inferred type of both the assignment target and the first argument.

Auto-correction is unsafe because replacing `T.bind` with `T.cast` removes the scope-wide type rebind of the first argument.

```ruby
# bad
foo = T.bind(self, Integer)

# good
foo = T.cast(self, Integer)
```

## Options

This rule has no options.

## Blind spots

None recorded.
