# Style/MultipleComparison

Avoid comparing a variable with multiple items in a conditional, use Array#include? instead.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks against comparing a variable with multiple items, where
`Array#include?`, `Set#include?` or a `case` could be used instead
to avoid code repetition.
It accepts comparisons of multiple method calls to avoid unnecessary method calls
by default. It can be configured by `AllowMethodComparison` option.

```ruby
# bad
a = 'a'
foo if a == 'a' || a == 'b' || a == 'c'

# good
a = 'a'
foo if ['a', 'b', 'c'].include?(a)

VALUES = Set['a', 'b', 'c'].freeze
# elsewhere...
foo if VALUES.include?(a)

case foo
when 'a', 'b', 'c' then foo
# ...
end

# accepted (but consider `case` as above)
foo if a == b.lightweight || a == b.heavyweight
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowMethodComparison | true |  | Whether a comparison whose "other" side is itself a method call is exempted. |
| ComparisonsThreshold | 2 |  | Minimum number of comparisons against the same variable before an offense is registered. |

## Blind spots

`variables` is deduplicated by source-text equality rather than upstream's structural `AST::Node#==` (which ignores location); every fixture's "variable" side always reads identically at each occurrence, so this never diverges in practice.
