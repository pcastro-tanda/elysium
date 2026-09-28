# Lint/DuplicateCaseCondition

Checks that there are no repeated conditions used in case 'when' expressions.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks that there are no repeated conditions used in case 'when' expressions.

```ruby
# bad
case x
when 'first'
  do_something
when 'first'
  do_something_else
end

# good
case x
when 'first'
  do_something
when 'second'
  do_something_else
end
```

## Options

This rule has no options.

## Blind spots

Conditions are compared by exact source text rather than RuboCop's true structural `Node#==`
(mirroring this codebase's established approximation in `Style/RedundantCondition` and
`Lint/SelfAssignment`); two conditions that are structurally identical but written with different
incidental formatting (extra parens, different whitespace, `'x'` vs `"x"`) are not flagged as
duplicates. Only plain `case`/`when` is checked, matching upstream's `on_case`; `case`/`in` pattern
matching (`CaseMatchNode`) is a distinct node kind upstream never subscribes to.
