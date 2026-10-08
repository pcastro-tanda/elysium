# Rails/ScopeArgs

Checks the arguments of ActiveRecord scopes.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for scope calls where it was passed a method (usually a scope) instead of a lambda/proc.

```ruby
# bad
scope :something, where(something: true)

# good
scope :something, -> { where(something: true) }
```

## Options

This rule has no options.

## Blind spots

None recorded.
