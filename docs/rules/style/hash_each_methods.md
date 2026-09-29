# Style/HashEachMethods

Use Hash#each_key and Hash#each_value.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for uses of `each_key` and `each_value` `Hash` methods.

NOTE: If you have an array of two-element arrays, you can put parentheses
around the block arguments to indicate that you're not working with a hash,
and suppress offenses.

This cop is unsafe because it cannot be guaranteed that the receiver is a
`Hash`. The `AllowedReceivers` configuration can mitigate, but not fully
resolve, this safety issue.

```ruby
# bad
hash.keys.each { |k| p k }
hash.each { |k, unused_value| p k }

# good
hash.each_key { |k| p k }

# bad
hash.values.each { |v| p v }
hash.each { |unused_key, v| p v }

# good
hash.each_value { |v| p v }
```

With `AllowedReceivers: ['execute']`:

```ruby
# good
execute(sql).keys.each { |v| p v }
execute(sql).values.each { |v| p v }
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedReceivers | `[]` |  | Receiver names (`AllowedReceivers#receiver_name`) whose `keys.each`/`values.each`/`each` chain is never flagged. |

## Blind spots

`hash_mutated?` (a receiver written to as `receiver[...] = ...` anywhere in the block body
suppresses the offense) is approximated by same-kind-and-same-source-text comparison rather than
upstream's full recursive AST equality; exact for the plain-identifier/bare-call receivers this
cop sees in practice. A block with two parameters directly on `_.keys.each`/`_.values.each`
(nonsensical Ruby, since `keys`/`values` yield one value) is not re-checked against
`each_arguments` after `kv_each` already matched, unlike upstream, which falls through when
`register_kv_offense` itself adds no offense (e.g. an allowed receiver).
