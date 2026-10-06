# Performance/Size

Use `size` instead of `count` for counting the number of elements in `Array` and `Hash`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies usages of `count` on an `Array` and `Hash` and change them to `size`.

```ruby
# bad
[1, 2, 3].count
(1..3).to_a.count
Array[*1..3].count
Array(1..3).count

# bad
{a: 1, b: 2, c: 3}.count
[[:foo, :bar], [1, 2]].to_h.count
Hash[*('a'..'z')].count
Hash(key: :value).count

# good
[1, 2, 3].size
(1..3).to_a.size
Array[*1..3].size
Array(1..3).size

# good
{a: 1, b: 2, c: 3}.size
[[:foo, :bar], [1, 2]].to_h.size
Hash[*('a'..'z')].size
Hash(key: :value).size

# good
[1, 2, 3].count { |e| e > 2 }
```

## Options

This rule has no options.

## Blind spots

None recorded.
