# Style/EvenOdd

Favor the use of `Integer#even?` && `Integer#odd?`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for places where `Integer#even?` or `Integer#odd?` can be used.

```ruby
# bad
if x % 2 == 0
end

# good
if x.even?
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
