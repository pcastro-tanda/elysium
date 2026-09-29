# Style/RedundantSelfAssignment

Checks for places where redundant assignments are made for in place modification methods.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for places where redundant assignments are made for in place
modification methods.

```ruby
# bad
args = args.concat(ary)
hash = hash.merge!(other)

# good
args.concat(foo)
args += foo
hash.merge!(other)

# good
foo.concat(ary)
```

## Options

This rule has no options.

## Blind spots

None recorded.
