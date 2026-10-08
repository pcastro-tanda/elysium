# Performance/Squeeze

Use `squeeze('a')` instead of `gsub(/a+/, 'a')`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `gsub(/a+/, 'a')` and `gsub!(/a+/, 'a')`
can be replaced by `squeeze('a')` and `squeeze!('a')`.

The `squeeze('a')` method is faster than `gsub(/a+/, 'a')`.

```ruby
# bad
str.gsub(/a+/, 'a')
str.gsub!(/a+/, 'a')

# good
str.squeeze('a')
str.squeeze!('a')
```

## Options

This rule has no options.

## Blind spots

None recorded.
