# Lint/LiteralAssignmentInCondition

Checks for literal assignments in the conditions of `if`, `while`, and `until`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

```ruby
# bad
if x = 42
  do_something
end

# good
if x == 42
  do_something
end

# good
if x = y
  do_something
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
