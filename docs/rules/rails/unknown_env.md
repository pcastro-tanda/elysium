# Rails/UnknownEnv

Use correct environment name.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks that environments called with `Rails.env` predicates exist.
By default the cop allows three environments which Rails ships with: `development`, `test`, and `production`. More can be added to the `Environments` config parameter.

```ruby
# bad
Rails.env.proudction?
Rails.env == 'proudction'
Rails.env != 'proudction'

# good
Rails.env.production?
Rails.env == 'production'
Rails.env != 'production'
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| Environments | `development`, `test`, `production` |  | Environment names that are considered known. |

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
