# Style/EachWithObject

Prefer `each_with_object` over `inject` or `reduce`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Looks for inject / reduce calls where the passed in object is
returned at the end and so could be replaced by each_with_object without
the need to return the object at the end.

However, we can't replace with each_with_object if the accumulator
parameter is assigned to within the block.

```ruby
# bad
[1, 2].inject({}) { |a, e| a[e] = e; a }

# good
[1, 2].each_with_object({}) { |e, a| a[e] = e }
```

## Options

This rule has no options.

## Blind spots

Only a block with exactly two required positional parameters and no other
parameter kind (optional/rest/post/keyword/block) is recognized as a
candidate, matching the given corpus; upstream's node pattern is looser
(`(args $_ $_)` matches any two parameters of any kind) but no fixture or
spec case exercises that.
