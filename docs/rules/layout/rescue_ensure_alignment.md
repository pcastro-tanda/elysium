# Layout/RescueEnsureAlignment

Align rescues and ensures correctly.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks whether the rescue and ensure keywords are aligned properly.

```ruby
# bad
begin
  something
  rescue
  puts 'error'
end

# good
begin
  something
rescue
  puts 'error'
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
