# Style/NestedTernaryOperator

Checks for nested ternary op expressions.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for nested ternary op expressions.

```ruby
# bad
a ? (b ? b1 : b2) : a2

# good
if a
  b ? b1 : b2
else
  a2
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
