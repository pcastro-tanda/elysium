# Style/FetchEnvVar

Suggests `ENV.fetch` for the replacement of `ENV[]`.

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
| AllowedVariables | `[]` |  | Environment variables to be excluded from the inspection. |
| DefaultToNil | true |  | When `true`, autocorrects `ENV["key"]` to `ENV.fetch("key", nil)`. When `false`, autocorrects to `ENV.fetch("key")`. |

## Blind spots

Structural-equality comparisons (`used_if_condition_in_body?`'s `child_nodes` checks) are approximated as same-source-text rather than a true AST-equality walk; a semantically identical but differently-written duplicate (e.g. different whitespace) would not be recognized as the same expression.
