# Rails/ActiveRecordCallbacksOrder

Order callback declarations in the order in which they will be executed.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that Active Record callbacks are declared in the order in which they will be executed.

```ruby
# bad
class Person < ApplicationRecord
  after_commit :after_commit_callback
  before_validation :before_validation_callback
end

# good
class Person < ApplicationRecord
  before_validation :before_validation_callback
  after_commit :after_commit_callback
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
