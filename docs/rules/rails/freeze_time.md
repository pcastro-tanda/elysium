# Rails/FreezeTime

Prefer `freeze_time` over `travel_to` with an argument of the current time.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies usages of `travel_to` with an argument of the current time and change them to use `freeze_time` instead.

This cop's autocorrection is unsafe because `freeze_time` just delegates to `travel_to` with a default `Time.now`, it is not strictly equivalent to `Time.now` if the argument of `travel_to` is the current time considering time zone.

```ruby
# bad
travel_to(Time.now)
travel_to(Time.new)
travel_to(DateTime.now)
travel_to(Time.current)
travel_to(Time.zone.now)
travel_to(Time.now.in_time_zone)
travel_to(Time.current.to_time)

# good
freeze_time
```

## Options

This rule has no options.

## Blind spots

None recorded.
