# Performance/DeletePrefix

Use `delete_prefix` instead of `gsub`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

In Ruby 2.5, `String#delete_prefix` has been added.

This cop identifies places where `gsub(/\Aprefix/, '')` and `sub(/\Aprefix/, '')`
can be replaced by `delete_prefix('prefix')`.

This cop has `SafeMultiline` configuration option that `true` by default because
`^prefix` is unsafe as it will behave incompatible with `delete_prefix`
for receiver is multiline string.

This cop is unsafe because `Pathname` has `sub` but not `delete_prefix`.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| SafeMultiline | true |  | Whether `^prefix` is treated as unsafe (multiline receivers). |

## Blind spots

None recorded.
