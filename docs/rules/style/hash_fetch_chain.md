# Style/HashFetchChain

Use `Hash#dig` instead of chained `fetch` calls.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Use `Hash#dig` instead of chaining potentially null `fetch` calls.

When `fetch(identifier, nil)` calls are chained on a hash, the expectation
is that each step in the chain returns either `nil` or another hash,
and in both cases, these can be simplified with a single call to `dig` with
multiple arguments.

If the 2nd parameter is `{}` or `Hash.new`, an offense will also be registered,
as long as the final call in the chain is a nil value. If a non-nil value is given,
the chain will not be registered as an offense, as the default value cannot be safely
given with `dig`.

@safety
This cop is unsafe because it cannot be guaranteed that the receiver
is a `Hash` or that `fetch` or `dig` have the expected standard implementation.

```ruby
# bad
hash.fetch('foo', nil)&.fetch('bar', nil)

# bad
# earlier members of the chain can return `{}` as long as the final `fetch`
# has `nil` as a default value
hash.fetch('foo', {}).fetch('bar', nil)

# good
hash.dig('foo', 'bar')

# ok - not handled by the cop since the final `fetch` value is non-nil
hash.fetch('foo', {}).fetch('bar', {})
```

## Options

This rule has no options.

## Blind spots

None recorded.
