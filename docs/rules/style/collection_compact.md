# Style/CollectionCompact

Use `{Array,Hash}#{compact,compact!}` instead of custom logic to reject nils.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for places where custom logic on rejection nils from arrays and hashes can be replaced with `{Array,Hash}#{compact,compact!}`.

It is unsafe by default because false positives may occur in the `nil` check of block arguments to the receiver object. Additionally, we can't know the type of the receiver object for sure, which may result in false positives as well.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedReceivers | `[]` |  | Allowed receiver names (`AllowedReceivers#receiver_name`) that are never flagged. |

## Blind spots

None recorded.
