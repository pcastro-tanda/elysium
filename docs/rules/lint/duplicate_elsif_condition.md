# Lint/DuplicateElsifCondition

Checks that there are no repeated conditions used in if 'elsif'.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks that there are no repeated conditions used in if 'elsif'.

```ruby
# bad
if x == 1
  do_something
elsif x == 1
  do_something_else
end

# good
if x == 1
  do_something
elsif x == 2
  do_something_else
end
```

## Options

This rule has no options.

## Blind spots

Conditions are compared by exact source text rather than RuboCop's true
structural `Node#==`: a genuinely equal condition written with different
incidental formatting (extra parens, different whitespace) is treated as
unequal.
