# Rails/AfterCommitOverride

Enforces that there is only one call to `after_commit` (and its aliases - `after_create_commit`, `after_update_commit`, and `after_destroy_commit`) with the same callback name per model.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks that only one `after_*_commit` hook is defined per callback name in a model, because a second hook with the same name overrides the first.

```ruby
# bad
class User < ApplicationRecord
  after_create_commit :log_action
  after_update_commit :log_action
end

# good
class User < ApplicationRecord
  after_save_commit :log_action
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
