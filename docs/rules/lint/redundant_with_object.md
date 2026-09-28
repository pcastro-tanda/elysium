# Lint/RedundantWithObject

Checks for redundant `with_object`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Checks for redundant `with_object`.

```ruby
# bad
ary.each_with_object([]) do |v|
  v
end

# good
ary.each do |v|
  v
end

# bad
ary.each.with_object([]) do |v|
  v
end

# good
ary.each do |v|
  v
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
