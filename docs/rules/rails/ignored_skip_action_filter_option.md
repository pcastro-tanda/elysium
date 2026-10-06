# Rails/IgnoredSkipActionFilterOption

Checks that `if` and `only` (or `except`) are not used together as options of `skip_*` action filter.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that `if` and `only` (or `except`) are not used together as options of `skip_*` action filter.

The `if` option will be ignored when `if` and `only` are used together. Similarly, the `except` option will be ignored when `if` and `except` are used together.

```ruby
# bad
class MyPageController < ApplicationController
  skip_before_action :login_required,
    only: :show, if: :trusted_origin?
end

# good
class MyPageController < ApplicationController
  skip_before_action :login_required,
    if: -> { trusted_origin? && action_name == "show" }
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
