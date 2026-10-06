# Rails/ToFormattedS

Checks for consistent uses of `to_fs` or `to_formatted_s`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for consistent uses of `to_fs` or `to_formatted_s`, depending on the cop's configuration.

```ruby
# EnforcedStyle: to_fs (default)
# bad
time.to_formatted_s(:db)

# good
time.to_fs(:db)
```

```ruby
# EnforcedStyle: to_formatted_s
# bad
time.to_fs(:db)

# good
time.to_formatted_s(:db)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `to_fs` | `to_fs`, `to_formatted_s` | Which of the two equivalent methods to use. |

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
