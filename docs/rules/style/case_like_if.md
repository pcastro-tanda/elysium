# Style/CaseLikeIf

Identifies places where `if-elsif` constructions can be replaced with `case-when`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

```ruby
# MinBranchesCount: 3 (default)
# bad
if status == :active
  perform_action
elsif status == :inactive || status == :hibernating
  check_timeout
elsif status == :invalid
  report_invalid
else
  final_action
end

# good
case status
when :active
  perform_action
when :inactive, :hibernating
  check_timeout
when :invalid
  report_invalid
else
  final_action
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| MinBranchesCount | 3 |  | The number of branches `if` needs to have to trigger this cop. |

## Blind spots

A `match`/`match?`/`=~` operand that is an `InterpolatedRegularExpressionNode` (a regexp literal with `#{...}` interpolation) is treated as never having named captures, since its content is only known at runtime; RuboCop's `regexp_parser`-based check has the same limitation for a dynamic pattern.
