# Lint/AmbiguousAssignment

Checks for mistyped shorthand assignments.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for mistyped shorthand assignments.

```ruby
# bad
x =- y
x =+ y
x =* y
x =! y

# good
x -= y # or x = -y
x += y # or x = +y
x *= y # or x = *y
x != y # or x = !y
```

## Options

This rule has no options.

## Blind spots

None recorded.
