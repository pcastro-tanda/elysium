# Rails/HttpPositionalArguments

Use keyword arguments instead of positional arguments in http method calls.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for the use of old-style attributes for HTTP request methods in tests, which Rails 5.0 deprecated (and 5.1 removed) in favour of keyword arguments.

```ruby
# bad
get :new, { user_id: 1 }

# good
get :new, params: { user_id: 1 }
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
