# Style/MultilineIfThen

Checks for uses of the `then` keyword in multi-line if statements.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of the `then` keyword in multi-line `if` statements.
In multi-line `if` statements, `then` is redundant because the newline
already separates the condition from the body.

```ruby
# bad
# This is considered bad practice.
if cond then
end

# good
# If statements can contain `then` on the same line.
if cond then a
elsif cond then b
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
