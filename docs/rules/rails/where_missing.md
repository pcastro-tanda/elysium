# Rails/WhereMissing

Use `where.missing(...)` to find missing relationship records.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Use `where.missing(...)` to find missing relationship records.

This cop is enabled in Rails 6.1 or higher.

```ruby
# bad
Post.left_joins(:author).where(authors: { id: nil })

# good
Post.where.missing(:author)
```

## Options

This rule has no options.

## Blind spots

None recorded.
