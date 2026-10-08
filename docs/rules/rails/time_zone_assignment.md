# Rails/TimeZoneAssignment

Prefer the usage of `Time.use_zone` instead of manually updating `Time.zone` value.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for the use of `Time.zone=` method.

The `zone` attribute persists for the rest of the Ruby runtime, potentially causing unexpected behavior at a later time. Using `Time.use_zone` ensures the code passed in the block is the only place `Time.zone` is affected. It eliminates the possibility of a `zone` sticking around longer than intended.

```ruby
# bad
Time.zone = 'EST'

# good
Time.use_zone('EST') do
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
