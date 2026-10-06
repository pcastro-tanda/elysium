# Rails/TopLevelHashWithIndifferentAccess

Identifies top-level `HashWithIndifferentAccess`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | warning |
| Fix | safe |
| Stability | stable |

Identifies top-level `HashWithIndifferentAccess`. This has been soft-deprecated since Rails 5.1.

```ruby
# bad
HashWithIndifferentAccess.new(foo: 'bar')

# good
ActiveSupport::HashWithIndifferentAccess.new(foo: 'bar')
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
