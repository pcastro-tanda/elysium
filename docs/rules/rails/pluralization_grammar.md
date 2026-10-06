# Rails/PluralizationGrammar

Checks for incorrect grammar when using methods like `3.day.ago`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for correct grammar when using `ActiveSupport`'s core extensions to the numeric classes.

```ruby
# bad
3.day.ago
1.months.ago
5.megabyte
1.gigabytes

# good
3.days.ago
1.month.ago
5.megabytes
1.gigabyte
```

## Options

This rule has no options.

## Blind spots

None recorded.
