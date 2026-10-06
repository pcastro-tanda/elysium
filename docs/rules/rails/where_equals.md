# Rails/WhereEquals

Pass conditions to `where` and `where.not` as a hash instead of manually constructing SQL.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies places where manually constructed SQL in `where` and `where.not` can be replaced with `where(attribute: value)` and `where.not(attribute: value)`.

This cop's autocorrection is unsafe because is may change SQL. See: https://github.com/rubocop/rubocop-rails/issues/403

```ruby
# bad
User.where('name = ?', 'Gabe')
User.where.not('name = ?', 'Gabe')
User.where('name = :name', name: 'Gabe')
User.where('name IS NULL')
User.where('name IN (?)', ['john', 'jane'])
User.where('name IN (:names)', names: ['john', 'jane'])
User.where('users.name = :name', name: 'Gabe')

# good
User.where(name: 'Gabe')
User.where.not(name: 'Gabe')
User.where(name: nil)
User.where(name: ['john', 'jane'])
User.where(users: { name: 'Gabe' })
```

## Options

This rule has no options.

## Blind spots

None recorded.
