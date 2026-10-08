# Rails/ActiveSupportOnLoad

Use `ActiveSupport.on_load(...)` to patch Rails framework classes.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Checks for Rails framework classes that are patched directly instead of using Active Support load hooks. Direct patching forcibly loads the framework referenced, using hooks defers loading until it's actually needed.

The autocorrection is unsafe: while using lazy load hooks is recommended, it changes the order in which is code is loaded and may reveal load order dependency bugs.

```ruby
# bad
ActiveRecord::Base.include(MyClass)

# good
ActiveSupport.on_load(:active_record) { include MyClass }
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
