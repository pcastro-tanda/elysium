# Rails/ApplicationRecord

Check that models subclass ApplicationRecord.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | nursery |

Checks that models subclass `ApplicationRecord` with Rails 5.0.

It is a common practice to define models inside migrations in order to retain forward compatibility by avoiding loading any application code. And so migration files are excluded by default for this cop.

This cop's autocorrection is unsafe because it may let the logic from `ApplicationRecord` sneak into an Active Record model that is not purposed to inherit logic common among other Active Record models.

```ruby
# good
class Rails5Model < ApplicationRecord
  # ...
end

# bad
class Rails4Model < ActiveRecord::Base
  # ...
end
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
