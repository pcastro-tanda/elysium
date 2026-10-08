# Rails/FindById

Favor the use of `find` over `where.take!`, `find_by!`, and `find_by_id!` when you need to retrieve a single record by primary key when you expect it to be found.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces that `ActiveRecord#find` is used instead of `where.take!`, `find_by!`, and `find_by_id!` to retrieve a single record by primary key when you expect it to be found.

```ruby
# bad
User.where(id: id).take!
User.find_by_id!(id)
User.find_by!(id: id)

# good
User.find(id)
```

## Options

This rule has no options.

## Blind spots

None recorded.
