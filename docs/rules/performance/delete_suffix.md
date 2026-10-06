# Performance/DeleteSuffix

Use `delete_suffix` instead of `gsub`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies places where `gsub(/suffix\z/, '')` and `sub(/suffix\z/, '')` can be replaced by `delete_suffix('suffix')`. With `SafeMultiline: false`, `suffix$` anchors are flagged too. Unsafe because `Pathname` has `sub` but not `delete_suffix`.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| SafeMultiline | true |  | Only flag `\z` anchors, not `$`, since `$` matches at every line end. |

## Blind spots

None recorded.
