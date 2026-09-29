# Style/RedundantCapitalW

Checks for %W when interpolation is not needed.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for usage of the `%W()` syntax when `%w()` would do.

```ruby
# bad
%W(cat dog pig)
%W[door wall floor]

# good
%w/swim run bike/
%w[shirt pants shoes]
%W(apple #{fruit} grape)
```

## Options

This rule has no options.

## Blind spots

None recorded.
