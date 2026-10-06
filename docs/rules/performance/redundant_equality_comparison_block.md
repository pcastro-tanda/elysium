# Performance/RedundantEqualityComparisonBlock

Checks for uses `Enumerable#all?`, `Enumerable#any?`, `Enumerable#one?`, or `Enumerable#none?` are compared with `===` or similar methods in block.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for uses `Enumerable#all?`, `Enumerable#any?`, `Enumerable#one?`, and `Enumerable#none?` compared with `===` or similar methods in a block.

By default, `Object#===` behaves the same as `Object#==`, but this behavior is overridden in subclasses (for example `Range#===`). `AllowRegexpMatch` is true by default because `regexp.match?('string')` in a block often changes to the opposite result when replaced by an argument.

This cop is unsafe because `===` and `==` do not always behave the same.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowRegexpMatch | true |  | Do not flag blocks using `=~` or `match?`. |

## Blind spots

None recorded.
