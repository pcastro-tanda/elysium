# Rails/TransactionExitStatement

Avoid the usage of `return`, `break` and `throw` in transaction blocks.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks for the use of exit statements (namely `return`, `break` and `throw`) in transactions. This is due to the eventual unexpected behavior when using ActiveRecord >= 7, where transactions exited using these statements are being rollbacked rather than committed (pre ActiveRecord 7 behavior).

As alternatives, it would be more intuitive to explicitly raise an error when rollback is desired, and to use `next` when commit is desired.

If you are defining custom transaction methods, you can configure it with `TransactionMethods`.

This cop is disabled on Rails >= 7.2 because transactions were restored to their historical behavior.

```ruby
# bad
ApplicationRecord.transaction do
  return if user.active?
end

# good
ApplicationRecord.transaction do
  # Rollback
  raise "User is active" if user.active?
end

# good
ApplicationRecord.transaction do
  # Commit
  next if user.active?
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| TransactionMethods | `[]` |  | Custom methods that open a transaction. |

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
