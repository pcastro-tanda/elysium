# Lint/RequireRelativeSelfPath

Checks for a file requiring itself with `require_relative`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

```ruby
# bad

# foo.rb
require_relative 'foo'
require_relative 'bar'

# good

# foo.rb
require_relative 'bar'
```

## Options

This rule has no options.

## Blind spots

None recorded.
