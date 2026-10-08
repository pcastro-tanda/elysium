# Performance/FlatMap

Use `Enumerable#flat_map` instead of `Enumerable#map...Array#flatten(1)` or `Enumerable#collect..Array#flatten(1)`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies usages of `map { ... }.flatten` and change them to use `flat_map { ... }` instead.

```ruby
# bad
[1, 2, 3, 4].map { |e| [e, e] }.flatten(1)
[1, 2, 3, 4].collect { |e| [e, e] }.flatten(1)

# good
[1, 2, 3, 4].flat_map { |e| [e, e] }
[1, 2, 3, 4].map { |e| [e, e] }.flatten
[1, 2, 3, 4].collect { |e| [e, e] }.flatten
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnabledForFlattenWithoutParams | false |  | Also register `flatten` without a level argument. |

## Blind spots

None recorded.
