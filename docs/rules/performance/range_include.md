# Performance/RangeInclude

Use `Range#cover?` instead of `Range#include?` (or `Range#member?`).

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies uses of `Range#include?` and `Range#member?`, which iterates over each item in a `Range` to see if a specified item is there. In contrast, `Range#cover?` simply compares the target item with the beginning and end points of the `Range`.

This cop is unsafe because `Range#include?` (or `Range#member?`) and `Range#cover?` are not equivalent behavior.

```ruby
# bad
('a'..'z').include?('b') # => true
('a'..'z').member?('b')  # => true

# good
('a'..'z').cover?('b') # => true
```

## Options

This rule has no options.

## Blind spots

None recorded.
