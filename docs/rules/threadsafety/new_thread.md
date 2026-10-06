# ThreadSafety/NewThread

Avoid starting new threads. Let a framework like Sidekiq handle the threads.

| | |
| --- | --- |
| Department | ThreadSafety |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Avoid starting new threads.

Let a framework like Sidekiq handle the threads.

```ruby
# bad
Thread.new { do_work }
```

## Options

This rule has no options.

## Blind spots

None recorded.
