# Style/PreferredHashMethods

Checks use of `has_key?` and `has_value?` Hash methods.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for uses of methods `Hash#has_key?` and `Hash#has_value?`, and suggests using `Hash#key?` and `Hash#value?` instead.

It is configurable to enforce the verbose method names, by using the `EnforcedStyle: verbose` configuration.

@safety
  This cop is unsafe because it cannot be guaranteed that the receiver is a `Hash` or responds to the replacement methods.

```ruby
# EnforcedStyle: short (default)
# bad
Hash#has_key?
Hash#has_value?

# good
Hash#key?
Hash#value?
```

```ruby
# EnforcedStyle: verbose
# bad
Hash#key?
Hash#value?

# good
Hash#has_key?
Hash#has_value?
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `short` | `short`, `verbose` | Whether to prefer the short (`key?`/`value?`) or verbose (`has_key?`/`has_value?`) `Hash` predicate method names. |

## Blind spots

None recorded.
