# Rails/ApplicationMailer

Check that mailers subclass ApplicationMailer.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks that mailers subclass `ApplicationMailer` with Rails 5.0.

This cop's autocorrection is unsafe because it may let the logic from `ApplicationMailer` sneak into a mailer that is not purposed to inherit logic common among other mailers.

```ruby
# good
class MyMailer < ApplicationMailer
  # ...
end

# bad
class MyMailer < ActionMailer::Base
  # ...
end
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
