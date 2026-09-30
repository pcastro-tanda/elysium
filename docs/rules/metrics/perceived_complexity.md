# Metrics/PerceivedComplexity

Checks that the perceived complexity of methods is not higher than the configured maximum.

| | |
| --- | --- |
| Department | Metrics |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Tries to produce a complexity score that's a measure of the complexity the reader experiences when looking at a method. For that reason it considers `when` nodes as something that doesn't add as much complexity as an `if` or a `&&`. Except if it's one of those special `case`/`when` constructs where there's no expression after `case`. Then the cop treats it as an `if`/`elsif`/`elsif`... and lets all the `when` nodes count. In contrast to the `CyclomaticComplexity` cop, this cop considers `else` nodes as adding complexity.

A `case`/`in` branch whose pattern is a simple literal (e.g. `in 1`, `in "red"`, `in 1..10`) or a constant/type (e.g. `in Integer`) and has no guard is just as easy to read as a `when` branch, so it is discounted the same way. Branches with structural patterns (e.g. array, hash, or find patterns), bindings, alternatives, or a guard add the full complexity of a decision point.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Max | 8 |  | Maximum perceived complexity allowed. |
| AllowedMethods | `[]` |  | Method names to exempt. |
| AllowedPatterns | `[]` |  | Patterns matching method names to exempt. |

## Blind spots

None recorded.
