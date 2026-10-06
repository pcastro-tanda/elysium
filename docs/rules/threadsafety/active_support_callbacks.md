# ThreadSafety/ActiveSupportCallbacks

Avoid mutating ActiveSupport callback chains at runtime.

| | |
| --- | --- |
| Department | ThreadSafety |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Avoid mutating ActiveSupport callback chains at runtime.

Calls such as `User.skip_callback` and `User.set_callback` mutate callback
chains at process scope.

```ruby
# bad
Site.skip_callback(:commit, :after, :after_owner_change)

# good
class User < ApplicationRecord
  skip_callback :commit, :after, :after_owner_change
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
