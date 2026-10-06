# Rails/ApplicationJob

Check that jobs subclass ApplicationJob.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks that jobs subclass `ApplicationJob` with Rails 5.0.

This cop's autocorrection is unsafe because it may let the logic from `ApplicationJob` sneak into a job that is not purposed to inherit logic common among other jobs.

```ruby
# good
class Rails5Job < ApplicationJob
  # ...
end

# bad
class Rails4Job < ActiveJob::Base
  # ...
end
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
