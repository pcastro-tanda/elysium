# Rails/UniqBeforePluck

Prefer the use of uniq or distinct before pluck.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Prefer the use of `distinct` before `pluck` instead of `uniq` after `pluck`: the database removes the duplicates rather than Ruby.

The autocorrection is unsafe because `uniq` after `pluck` may be applied to an Array rather than a relation, and `distinct` before `pluck` can change how the query is ordered.

```ruby
# bad
Model.pluck(:id).uniq

# good
Model.distinct.pluck(:id)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `conservative` | `conservative`, `aggressive` | `conservative` only checks `pluck` called on a constant; `aggressive` checks every receiver. |

## Blind spots

None recorded.
