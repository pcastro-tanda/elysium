# Rails/SelectMap

Checks for uses of `select(:column_name)` with `map(&:column_name)`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for uses of `select(:column_name)` with `map(&:column_name)`.
These can be replaced with `pluck(:column_name)`.

There also should be some performance improvement since it skips instantiating the model class for matches.

This cop is unsafe because the model might override the attribute getter. Additionally, the model's `after_initialize` hooks are skipped when using `pluck`.

```ruby
# bad
Model.select(:column_name).map(&:column_name)

# good
Model.pluck(:column_name)
```

## Options

This rule has no options.

## Blind spots

None recorded.
