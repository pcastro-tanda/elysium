# Rails/WhereNot

Use `where.not(...)` instead of manually constructing negated SQL in `where`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Identifies places where manually constructed SQL in `where` can be replaced with `where.not(...)`.

```ruby
# bad
User.where('name != ?', 'Gabe')
User.where('name != :name', name: 'Gabe')
User.where('name <> ?', 'Gabe')
User.where('name <> :name', name: 'Gabe')
User.where('name IS NOT NULL')
User.where('name NOT IN (?)', ['john', 'jane'])
User.where('name NOT IN (:names)', names: ['john', 'jane'])
User.where('users.name != :name', name: 'Gabe')

# good
User.where.not(name: 'Gabe')
User.where.not(name: nil)
User.where.not(name: ['john', 'jane'])
User.where.not(users: { name: 'Gabe' })
```

## Options

This rule has no options.

## Blind spots

None recorded.
