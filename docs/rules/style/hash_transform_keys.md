# Style/HashTransformKeys

Prefer `transform_keys` over `each_with_object`, `map`, or `to_h`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Looks for uses of `each_with_object({}) {...}`, `map {...}.to_h`, and `Hash[map {...}]` that are actually just transforming the keys of a hash, and tries to use a simpler & faster call to `transform_keys` instead.

It should only be enabled on Ruby version 2.5 or newer (`transform_keys` was added in Ruby 2.5).

This cop is unsafe, as it can produce false positives if we are transforming an enumerable of key-value-like pairs that isn't actually a hash, e.g.: `[[k1, v1], [k2, v2], ...]`.

## Options

This rule has no options.

## Blind spots

None recorded.
