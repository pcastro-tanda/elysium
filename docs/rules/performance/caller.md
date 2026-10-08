# Performance/Caller

Use `caller(n..n)` instead of `caller`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `caller[n]` can be replaced by `caller(n..n).first`.

```ruby
# bad
caller[1]
caller.first
caller_locations[1]
caller_locations.first

# good
caller(2..2).first
caller(1..1).first
caller_locations(2..2).first
caller_locations(1..1).first
```

## Options

This rule has no options.

## Blind spots

An integer argument outside the 32-bit range is not recognized.
