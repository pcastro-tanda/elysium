# Style/RedundantSort

Use `min` instead of `sort.first`, `max_by` instead of `sort_by...last`, etc.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies instances of sorting and then taking only the first or last element. The same behavior can be accomplished without a relatively expensive sort by using `Enumerable#min` instead of sorting and taking the first element and `Enumerable#max` instead of sorting and taking the last element. Similarly, `Enumerable#min_by` and `Enumerable#max_by` can replace `Enumerable#sort_by` calls after which only the first or last element is used.

This cop is unsafe, because `sort...last` and `max` may not return the same element in all cases: where there are multiple elements for which `a <=> b == 0`, or where the transformation done by the `sort_by` block has the same result, `sort.last` returns the last such element but `max` returns the first.

```ruby
# bad
[2, 1, 3].sort.first
[2, 1, 3].sort[0]
[2, 1, 3].sort.at(0)
[2, 1, 3].sort.slice(0)

# good
[2, 1, 3].min

# bad
[2, 1, 3].sort.last
[2, 1, 3].sort[-1]

# good
[2, 1, 3].max

# bad
arr.sort_by(&:foo).first

# good
arr.min_by(&:foo)
```

## Options

This rule has no options.

## Blind spots

None recorded.
