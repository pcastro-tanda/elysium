# Rails/WhereRange

Use ranges in `where` instead of manually constructing SQL.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies places where manually constructed SQL in `where` can be replaced with ranges.

This cop's autocorrection is unsafe because it can change the query by explicitly attaching the column to the wrong table. For example, `Booking.joins(:events).where('end_at < ?', Time.current)` will correctly implicitly attach the `end_at` column to the `events` table. But when autocorrected to `Booking.joins(:events).where(end_at: ...Time.current)`, it will now be incorrectly explicitly attached to the `bookings` table.

```ruby
# bad
User.where('age >= ?', 18)
User.where.not('age >= ?', 18)
User.where('age < ?', 18)
User.where('age >= ? AND age < ?', 18, 21)
User.where('age >= :start', start: 18)
User.where('users.age >= ?', 18)

# good
User.where(age: 18..)
User.where.not(age: 18..)
User.where(age: ...18)
User.where(age: 18...21)
User.where(users: { age: 18.. })

# good
# There are no beginless ranges in ruby.
User.where('age > ?', 18)
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
