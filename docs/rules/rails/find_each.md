# Rails/FindEach

Prefer all.find_each over all.each.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Identifies usages of `all.each` and change them to use `all.find_each` instead.

This cop is unsafe if the receiver object is not an Active Record object. Also, `all.each` returns an `Array` instance and `all.find_each` returns nil, so the return values are different.

```ruby
# bad
User.all.each

# good
User.all.find_each
```

With `AllowedMethods: ['order']` or `AllowedPatterns: ['order']`, `User.order(:foo).each` is left alone.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `order`, `limit`, `select`, `lock` |  | Methods that don't work well with `find_each`. |
| AllowedPatterns | `[]` |  | Regular expressions of method names that don't work well with `find_each`. |

## Blind spots

None recorded.
