# Rails/EagerEvaluationLogMessage

Checks that blocks are used for interpolated strings passed to `Rails.logger.debug`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that blocks are used for interpolated strings passed to `Rails.logger.debug`.

By default, Rails production environments use the `:info` log level. At the `:info` log level, `Rails.logger.debug` statements do not result in log output. However, Ruby must eagerly evaluate interpolated string arguments passed as method arguments. Passing a block to `Rails.logger.debug` prevents costly evaluation of interpolated strings when no output would be produced anyway.

```ruby
# bad
Rails.logger.debug "The time is #{Time.zone.now}."

# good
Rails.logger.debug { "The time is #{Time.zone.now}." }
```

## Options

This rule has no options.

## Blind spots

None recorded.
