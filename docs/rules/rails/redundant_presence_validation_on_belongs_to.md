# Rails/RedundantPresenceValidationOnBelongsTo

Checks for redundant presence validation on `belongs_to` association.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Since Rails 5.0 the default for `belongs_to` is `optional: false` unless `config.active_record.belongs_to_required_by_default` is explicitly set to `false`. The presence validator is added automatically, and explicit presence validation is redundant.

This cop's autocorrection is unsafe because it changes the default error message from "can't be blank" to "must exist".

```ruby
# bad
belongs_to :user
validates :user, presence: true

# bad
belongs_to :user
validates :user_id, presence: true

# bad
belongs_to :author, foreign_key: :user_id
validates :user_id, presence: true

# good
belongs_to :user

# good
belongs_to :author, foreign_key: :user_id
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
