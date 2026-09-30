# Metrics/CyclomaticComplexity

Checks that the cyclomatic complexity of methods is not higher than the configured maximum.

| | |
| --- | --- |
| Department | Metrics |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

The cyclomatic complexity is the number of linearly independent paths through a method. The algorithm counts decision points and adds one.

An `if` statement (or `unless` or `?:`) increases the complexity by one. An `else` branch does not, since it doesn't add a decision point. The `&&` operator (or keyword `and`) can be converted to a nested `if` statement, and `||`/`or` is shorthand for a sequence of `if`s, so they also add one. Loops can be said to have an exit condition, so they add one. Blocks that are calls to builtin iteration methods (e.g. `ary.map { ... }`) also add one, others are ignored.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 7 |  | Maximum cyclomatic complexity allowed. |
| AllowedMethods | `[]` |  | Method names to exempt. |
| AllowedPatterns | `[]` |  | Patterns matching method names to exempt. |

## Blind spots

None recorded.
