# Rails/EnumHash

Prefer hash syntax over array syntax when defining enums.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Looks for enums written with array syntax.

When using array syntax, adding an element in a position other than the last causes all previous definitions to shift. Explicitly specifying the value for each key prevents this from happening.

```ruby
# bad
enum :status, [:active, :archived]

# good
enum :status, { active: 0, archived: 1 }

# bad
enum status: [:active, :archived]

# good
enum status: { active: 0, archived: 1 }
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
