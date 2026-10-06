# Rails/ExpandedDateRange

Checks for expanded date range.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for expanded date range. It checks `all_day`, `all_week`, `all_month`, `all_quarter` and `all_year` methods.

```ruby
# bad
created_at: date.beginning_of_day..date.end_of_day
created_at: date.beginning_of_week..date.end_of_week
created_at: date.beginning_of_month..date.end_of_month
created_at: date.beginning_of_quarter..date.end_of_quarter
created_at: date.beginning_of_year..date.end_of_year

# good
created_at: date.all_day
created_at: date.all_week
created_at: date.all_month
created_at: date.all_quarter
created_at: date.all_year
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
