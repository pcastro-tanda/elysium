# Performance/RedundantMerge

Use Hash#[]=, rather than Hash#merge! with a single key-value pair.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies places where `Hash#merge!` can be replaced by `Hash#[]=`.
You can set the maximum number of key-value pairs to consider
an offense with `MaxKeyValuePairs`.

This cop is unsafe because RuboCop cannot determine if the
receiver of `merge!` is actually a hash or not.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| MaxKeyValuePairs | 2 |  | Max number of key-value pairs to consider an offense. |

## Blind spots

None recorded.
