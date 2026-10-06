# Rails/ApplicationController

Check that controllers subclass ApplicationController.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks that controllers subclass `ApplicationController`.

This cop's autocorrection is unsafe because it may let the logic from `ApplicationController` sneak into a controller that is not purposed to inherit logic common among other controllers.

```ruby
# good
class MyController < ApplicationController
  # ...
end

# bad
class MyController < ActionController::Base
  # ...
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
