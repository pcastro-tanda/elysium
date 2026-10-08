# Style/EndlessMethod

Avoid the use of multi-lined endless method definitions.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |



## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `allow_single_line` | `allow_single_line`, `allow_always`, `disallow`, `require_single_line`, `require_always` | Whether/when endless method definitions are required, allowed, or disallowed. |

## Blind spots

Reads `Layout/IndentationWidth`'s `Width` and `Layout/LineLength`'s `Max`/`Enabled` as peer options, matching upstream's own cross-cop reads.
