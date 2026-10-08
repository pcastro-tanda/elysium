# Rails/EnumSyntax

Use positional arguments over keyword arguments when defining enums.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Looks for enums written with keyword arguments syntax.

Defining enums with keyword arguments syntax is deprecated and will be removed in Rails 8.0. Positional arguments should be used instead:

```ruby
# bad
enum status: { active: 0, archived: 1 }, _prefix: true

# good
enum :status, { active: 0, archived: 1 }, prefix: true
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
