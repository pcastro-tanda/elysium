# Lint/IncompatibleIoSelectWithFiberScheduler

Checks for `IO.select` that is incompatible with Fiber Scheduler.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for `IO.select` that is incompatible with Fiber Scheduler since Ruby 3.0.

When an array of IO objects waiting for an exception (the third argument of
`IO.select`) is used as an argument, there is no alternative API, so offenses
are not registered.

```ruby
# bad
IO.select([io], [], [], timeout)

# good
io.wait_readable(timeout)

# bad
IO.select([], [io], [], timeout)

# good
io.wait_writable(timeout)
```

## Options

This rule has no options.

## Blind spots

None recorded.
