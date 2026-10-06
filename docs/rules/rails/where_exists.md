# Rails/WhereExists

Prefer `exists?(...)` over `where(...).exists?`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Enforces consistent style when using `exists?`.

When EnforcedStyle is 'exists' the cop enforces `exists?(...)` over `where(...).exists?`; when 'where' it enforces `where(...).exists?` over `exists?(...)`.

```ruby
# EnforcedStyle: exists (default)
# bad
User.where(name: 'john').exists?
User.where(['name = ?', 'john']).exists?
User.where('name = ?', 'john').exists?
user.posts.where(published: true).exists?

# good
User.exists?(name: 'john')
User.where('length(name) > 10').exists?
user.posts.exists?(published: true)
```

Unsafe autocorrection: `Author.includes(:articles).where(articles: {id: id}).exists?` performs `eager_load` behavior, while `Author.includes(:articles).exists?(articles: {id: id})` performs `preload` behavior and raises `ActiveRecord::StatementInvalid`.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `exists` | `exists`, `where` | Whether to prefer `exists?(...)` or `where(...).exists?`. |

## Blind spots

None recorded.
