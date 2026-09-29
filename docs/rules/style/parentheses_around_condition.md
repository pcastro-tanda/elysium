# Style/ParenthesesAroundCondition

Don't use parentheses around the condition of an if/unless/while.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

`AllowSafeAssignment` option for safe assignment. By safe assignment we mean putting parentheses around an assignment to indicate "I know I'm using an assignment as a condition. It's not a mistake."

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowSafeAssignment | true |  | Whether an assignment wrapped in parentheses (e.g. `if (test = 10)`) is allowed. |
| AllowInMultilineConditions | false |  | Whether parentheses are allowed around a multiline condition. |

## Blind spots

`ParenthesesCorrector`'s comment-above-close-paren/chained-call preservation and orphaned-trailing-comma handling are not reproduced, since no fixture exercises those cases.
