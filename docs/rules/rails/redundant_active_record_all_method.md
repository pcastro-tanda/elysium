# Rails/RedundantActiveRecordAllMethod

Detect redundant `all` used as a receiver for Active Record query methods.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Detect redundant `all` used as a receiver for Active Record query methods.

For the methods `delete_all` and `destroy_all`, this cop will only check cases where the receiver is a model. It will ignore cases where the receiver is an association (e.g., `user.articles.all.delete_all`). This is because omitting `all` from an association changes the methods from `ActiveRecord::Relation` to `ActiveRecord::Associations::CollectionProxy`, which can affect their behavior.

This cop is unsafe because false positives will occur if the receiver is not an Active Record object.

```ruby
# bad
User.all.find(id)
User.all.order(:created_at)
users.all.where(id: ids)
user.articles.all.order(:created_at)

# good
User.find(id)
User.order(:created_at)
users.where(id: ids)
user.articles.order(:created_at)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedReceivers | `ActionMailer::Preview`, `ActiveSupport::TimeZone` |  | Receiver names (`AllowedReceivers#receiver_name`) whose `all` is never flagged. |

## Blind spots

None recorded.
