# Rails/DurationArithmetic

Do not use duration as arithmetic operand with `Time.current`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks if a duration is added to or subtracted from `Time.current`.

```ruby
# bad
Time.current - 1.minute
Time.zone.now + 2.days

# good
1.minute.ago
2.days.from_now
```

## Options

This rule has no options.

## Blind spots

None recorded.
