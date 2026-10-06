# Rails/MailerName

Mailer should end with `Mailer` suffix.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Enforces that mailer names end with `Mailer` suffix.

Without the `Mailer` suffix it isn't immediately apparent what's a mailer and which views are related to the mailer.

The autocorrection is unsafe because renaming a constant is always an unsafe operation.

```ruby
# bad
class User < ActionMailer::Base
end

class User < ApplicationMailer
end

# good
class UserMailer < ActionMailer::Base
end

class UserMailer < ApplicationMailer
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
