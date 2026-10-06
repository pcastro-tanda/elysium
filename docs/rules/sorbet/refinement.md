# Sorbet/Refinement

Checks for the use of Ruby Refinements library. Refinements add complexity and incur a performance penalty that can be significant for large code bases. They are also not supported by Sorbet.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for the use of Ruby Refinements library. Refinements add
complexity and incur a performance penalty that can be significant
for large code bases. Good examples are cases of unrelated
methods that happen to have the same name as these module methods.

```ruby
# bad
module Foo
  refine(Date) do
  end
end

# bad
module Foo
  using(Date) do
  end
end

# good
module Foo
  bar.refine(Date)
end

# good
module Foo
  bar.using(Date)
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
