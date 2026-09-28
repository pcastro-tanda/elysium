# Style/HashTransformValues

Checks for uses of `each_with_object`, `map`, and `to_h` that are actually just transforming the values of a hash, and prefers `transform_values` instead.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | nursery |

Looks for uses of `each_with_object({})`, `map { ... }.to_h`, and `Hash[_.map { ... }]` that are actually just transforming the values of a hash, and tries to use a simpler & faster call to `transform_values` instead.

This cop is unsafe, as it can produce false positives if we are transforming an enumerable of key-value-like pairs that isn't actually a hash, e.g.: `[[k1, v1], [k2, v2], ...]`.

## Options

This rule has no options.

## Blind spots

None recorded.
