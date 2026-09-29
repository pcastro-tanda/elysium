# Style/ModuleFunction

Checks for use of `extend self` or `module_function` in a module.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for use of `extend self` or `module_function` in a module.

Supported styles are: `module_function` (default), `extend_self` and `forbidden`.

A couple of things to keep in mind:

* `forbidden` style prohibits the usage of both styles
* in default mode (`module_function`), the cop won't be activated when the
  module contains any private methods

```ruby
# EnforcedStyle: module_function (default)
# bad
module Test
  extend self
  # ...
end

# good
module Test
  module_function
  # ...
end

# good
module Test
  extend self
  # ...
  private
  # ...
end

# EnforcedStyle: extend_self
# bad
module Test
  module_function
  # ...
end

# good
module Test
  extend self
  # ...
end

# EnforcedStyle: forbidden
# bad
module Test
  module_function
  # ...
end

# bad
module Test
  extend self
  # ...
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `module_function` | `module_function`, `extend_self`, `forbidden` | Which of `extend self`/`module_function` to enforce (or forbid both). |

## Blind spots

Autocorrection is unsafe (and is disabled by default) because `extend self`
and `module_function` do not behave exactly the same.
