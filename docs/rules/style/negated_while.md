# Style/NegatedWhile

Checks for uses of while with a negated condition.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for uses of `while` with a negated condition.

```ruby
# bad
while !foo
  bar
end

# good
until foo
  bar
end

# bad
bar until !foo

# good
bar while foo
bar while !foo && baz
```

## Options

This rule has no options.

## Blind spots

None recorded.
