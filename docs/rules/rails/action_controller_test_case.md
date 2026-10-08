# Rails/ActionControllerTestCase

Use `ActionDispatch::IntegrationTest` instead of `ActionController::TestCase`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Using `ActionController::TestCase` is discouraged and should be replaced by `ActionDispatch::IntegrationTest`. Controller tests are too close to the internals of a controller whereas integration tests mimic the browser/user.

This cop's autocorrection is unsafe because the API of each test case class is different. Make sure to update each test of your controller test cases after changing the superclass.

```ruby
# bad
class MyControllerTest < ActionController::TestCase
end

# good
class MyControllerTest < ActionDispatch::IntegrationTest
end
```

## Options

This rule has no options.

## Blind spots

Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.
