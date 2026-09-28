# Style/MultilineTernaryOperator

Avoid multi-line ?: (the ternary operator); use if/unless instead.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for multi-line ternary op expressions.

NOTE: `return if ... else ... end` is syntax error. If `return` is used before
multiline ternary operator expression, it will be autocorrected to single-line
ternary operator. The same is true for `break`, `next`, and method call.

```ruby
# bad
a = cond ?
  b : c
a = cond ? b :
    c
a = cond ?
    b :
    c

return cond ?
       b :
       c

# good
a = cond ? b : c
a = if cond
  b
else
  c
end

return cond ? b : c
```

## Options

This rule has no options.

## Blind spots

None recorded.
