# Performance/ConcurrentMonotonicTime

Use `Process.clock_gettime(Process::CLOCK_MONOTONIC)` instead of `Concurrent.monotonic_time`.

| | |
| --- | --- |
| Department | Performance |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where `Concurrent.monotonic_time` can be replaced by `Process.clock_gettime(Process::CLOCK_MONOTONIC)`.

## Options

This rule has no options.

## Blind spots

None recorded.
