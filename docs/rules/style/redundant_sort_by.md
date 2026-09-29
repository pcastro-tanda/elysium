# Style/RedundantSortBy

Use `sort` instead of `sort_by { |x| x }`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `sort_by { ... }` can be replaced by `sort`.

```ruby
# bad
array.sort_by { |x| x }
array.sort_by do |var|
var
end

# good
array.sort
```

## Options

This rule has no options.

## Blind spots

None recorded.
