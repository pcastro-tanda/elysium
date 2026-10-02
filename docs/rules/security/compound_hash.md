# Security/CompoundHash

Checks for `hash` implementations that combine values manually instead of delegating to `Array#hash`.

| | |
| --- | --- |
| Department | Security |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for implementations of the `hash` method which combine
values using custom logic instead of delegating to `Array#hash`.

Manually combining hashes is error prone and hard to follow, especially
when there are many values. Poor implementations may also introduce
performance or security concerns if they are prone to collisions.
Delegating to `Array#hash` is clearer and safer, although it might be slower
depending on the use case.

```ruby
# bad
def hash
  @foo ^ @bar
end

# good
def hash
  [@foo, @bar].hash
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
