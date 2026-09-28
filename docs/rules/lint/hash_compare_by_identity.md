# Lint/HashCompareByIdentity

Checks for hashes being keyed by objects' `object_id`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Prefer using `Hash#compare_by_identity` rather than using `object_id` for hash keys.

This cop looks for hashes being keyed by objects' `object_id`, using one of these methods:
`key?`, `has_key?`, `fetch`, `[]` and `[]=`.

```ruby
# bad
hash = {}
hash[foo.object_id] = :bar
hash.key?(baz.object_id)

# good
hash = {}.compare_by_identity
hash[foo] = :bar
hash.key?(baz)
```

## Options

This rule has no options.

## Blind spots

This cop is unsafe: although unlikely, the hash could store both object ids and other values that
need to be compared by value, and thus could be a false positive. It also cannot guarantee that the
receiver of one of the methods (`key?`, etc.) is actually a hash.
