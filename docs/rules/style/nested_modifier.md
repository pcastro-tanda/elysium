# Style/NestedModifier

Avoid using nested modifiers.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for nested use of if, unless, while and until in their modifier form.

```ruby
# bad
something if a if b

# good
something if b && a
```

## Options

This rule has no options.

## Blind spots

None recorded.
