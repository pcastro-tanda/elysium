# Rails/NegateInclude

Prefer `collection.exclude?(obj)` over `!collection.include?(obj)`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Enforces the use of `collection.exclude?(obj)` over `!collection.include?(obj)`.

This cop is unsafe because false positive will occur for receiver objects that do not have an `exclude?` method. (e.g. `IPAddr`)

```ruby
# bad
!array.include?(2)
!hash.include?(:key)

# good
array.exclude?(2)
hash.exclude?(:key)
```

## Options

This rule has no options.

## Blind spots

None recorded.
