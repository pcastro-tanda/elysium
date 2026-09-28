# Lint/IdentityComparison

Prefer `equal?` over `==` when comparing `object_id`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

`Object#equal?` is provided to compare objects for identity, and in contrast
`Object#==` is provided for the purpose of doing value comparison.

```ruby
# bad
foo.object_id == bar.object_id
foo.object_id != baz.object_id

# good
foo.equal?(bar)
!foo.equal?(baz)
```

## Options

This rule has no options.

## Blind spots

None recorded.
