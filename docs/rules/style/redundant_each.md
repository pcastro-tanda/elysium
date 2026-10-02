# Style/RedundantEach

Checks for redundant `each`.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for redundant `each`.

```ruby
# bad
array.each.each { |v| do_something(v) }

# good
array.each { |v| do_something(v) }

# bad
array.each.each_with_index { |v, i| do_something(v, i) }

# good
array.each.with_index { |v, i| do_something(v, i) }
array.each_with_index { |v, i| do_something(v, i) }

# bad
array.each.each_with_object { |v, o| do_something(v, o) }

# good
array.each.with_object { |v, o| do_something(v, o) }
array.each_with_object { |v, o| do_something(v, o) }
```

## Options

This rule has no options.

## Blind spots

This cop is unsafe, as it can produce false positives if the receiver is
not an `Enumerator` (matching upstream's own `@safety` note).
