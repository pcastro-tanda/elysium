# Style/MinMax

Use `Enumerable#minmax` instead of `Enumerable#min` and `Enumerable#max` in conjunction.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for potential uses of `Enumerable#minmax`.

```ruby
# bad
bar = [foo.min, foo.max]
return foo.min, foo.max

# good
bar = foo.minmax
return foo.minmax
```

## Options

This rule has no options.

## Blind spots

None recorded.
