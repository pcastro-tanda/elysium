# Rails/EnvLocal

Use `Rails.env.local?` instead of `Rails.env.development? || Rails.env.test?`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for usage of `Rails.env.development? || Rails.env.test?` which can be replaced by `Rails.env.local?`, introduced in Rails 7.1.

```ruby
# bad
Rails.env.development? || Rails.env.test?

# good
Rails.env.local?
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
