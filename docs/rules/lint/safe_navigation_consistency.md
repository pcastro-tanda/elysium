# Lint/SafeNavigationConsistency

Check to make sure that if safe navigation is used in an `&&` or `||` condition, consistent and appropriate safe navigation, without excess or deficiency, is used for all method calls on the same object.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks that if safe navigation is used in an `&&` or `||` condition, consistent and appropriate safe navigation, without excess or deficiency, is used for all method calls on the same object.

# Examples

```ruby
# bad
foo&.bar && foo&.baz

# good
foo&.bar && foo.baz

# bad
foo.bar && foo&.baz

# good
foo.bar && foo.baz

# bad
foo&.bar || foo.baz

# good
foo&.bar || foo&.baz

# bad
foo.bar || foo&.baz

# good
foo.bar || foo.baz

# bad
foo&.bar && (foobar.baz || foo&.baz)

# good
foo&.bar && (foobar.baz || foo.baz)
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| AllowedMethods | `present?`, `blank?`, `presence`, `try`, `try!` |  | Allowed methods that will be always accepted regardless of whether they're inconsistent with the rest of a group of calls to the same object. |

## Blind spots

None recorded.
