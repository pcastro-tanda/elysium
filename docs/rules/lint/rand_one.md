# Lint/RandOne

Checks for `rand(1)` calls.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for `rand(1)` calls.
Such calls always return `0`.

```ruby
# bad

rand 1
Kernel.rand(-1)
rand 1.0
rand(-1.0)

# good

0 # just use 0 instead
```

## Options

This rule has no options.

## Blind spots

None recorded.
