# Rails/RedundantForeignKey

Checks for associations where the `:foreign_key` option is redundant.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Detects cases where the `:foreign_key` option on associations is redundant.

```ruby
# bad
class Post
  has_many :comments, foreign_key: 'post_id'
end

class Comment
  belongs_to :post, foreign_key: 'post_id'
end

# good
class Post
  has_many :comments
end

class Comment
  belongs_to :author, foreign_key: 'user_id'
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
