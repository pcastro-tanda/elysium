# Rails/RedundantReceiverInWithOptions

Checks for redundant receiver in `with_options`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for redundant receiver in `with_options`. Receiver is implicit from Rails 4.2 or higher.

```ruby
# bad
class Account < ApplicationRecord
  with_options dependent: :destroy do |assoc|
    assoc.has_many :customers
  end
end

# good
class Account < ApplicationRecord
  with_options dependent: :destroy do
    has_many :customers
  end
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
