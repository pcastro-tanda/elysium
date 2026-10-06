# Performance/InefficientHashSearch

Use `key?` or `value?` instead of `keys.include?` or `values.include?`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for inefficient searching of keys and values within hashes. `Hash#keys.include?` allocates an array and does an O(n) search, while `Hash#key?` is O(1); `Hash#values.include?` allocates an array, while `Hash#value?` does not. Unsafe because the receiver may not be a hash.

## Options

This rule has no options.

## Blind spots

None recorded.
