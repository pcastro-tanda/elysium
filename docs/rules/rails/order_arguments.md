# Rails/OrderArguments

Prefer symbol arguments over strings in `order` method.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Prefer symbol arguments over strings in `order` method.

Cop is unsafe because the receiver might not be an Active Record query.

```ruby
# bad
User.order('name')
User.order('name DESC')

# good
User.order(:name)
User.order(name: :desc)
```

## Options

This rule has no options.

## Blind spots

None recorded.
