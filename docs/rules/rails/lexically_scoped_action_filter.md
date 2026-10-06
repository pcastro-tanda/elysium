# Rails/LexicallyScopedActionFilter

Checks that methods specified in the filter's `only` or `except` options are defined within the same class or module.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Checks that methods specified in the filter's `only` or `except` options are defined within the same class or module.

You can technically specify methods of superclass or methods added by mixins on the filter, but these can confuse developers. If you specify methods that are defined in other classes or modules, you should define the filter in that class or module.

```ruby
# bad
class LoginController < ApplicationController
  before_action :require_login, only: %i[index settings logout]

  def index
  end
end

# good
class LoginController < ApplicationController
  before_action :require_login, only: %i[index settings logout]

  def index
  end

  def settings
  end

  def logout
  end
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
