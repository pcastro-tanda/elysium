# Lint/EmptyExpression

Checks for the presence of empty expressions.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for the presence of empty expressions.

```ruby
# bad

foo = ()
if ()
  bar
end

# good

foo = (some_expression)
if (some_expression)
  bar
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
