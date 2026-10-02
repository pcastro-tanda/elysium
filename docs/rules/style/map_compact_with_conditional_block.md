# Style/MapCompactWithConditionalBlock

Prefer `select` or `reject` over `map { ... }.compact`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

This cop also handles `filter_map { ... }`, similar to `map { ... }.compact`.

@safety
  This cop is unsafe because `compact` also removes `nil` elements that were already present in the receiver, whereas `select`/`reject` keep them. The result therefore differs when the collection contains `nil`:

  [nil, 1].map { |e| e if e }.compact # => [1]
  [nil, 1].select { |e| e }           # => [nil, 1]

## Options

This rule has no options.

## Blind spots

None recorded.
