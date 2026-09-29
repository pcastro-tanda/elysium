# Style/RedundantConditional

Don't return true/false from a conditional.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for redundant returning of true/false in conditionals.

```ruby
# bad
x == y ? true : false

# bad
if x == y
  true
else
  false
end

# good
x == y

# bad
x == y ? false : true

# good
x != y
```

## Options

This rule has no options.

## Blind spots

None recorded.
