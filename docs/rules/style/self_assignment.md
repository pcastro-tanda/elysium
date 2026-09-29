# Style/SelfAssignment

Checks for places where self-assignment shorthand should have been used.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of the shorthand for self-assignment.

```ruby
# bad
x = x + 1

# good
x += 1
```

## Options

This rule has no options.

## Blind spots

None recorded.
