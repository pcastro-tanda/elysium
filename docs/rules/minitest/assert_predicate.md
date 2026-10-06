# Minitest/AssertPredicate

This cop enforces the test to use `assert_predicate` instead of using `assert(obj.a_predicate_method?)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `assert_predicate` instead of using `assert(obj.a_predicate_method?)`.

```ruby
# bad
assert(obj.one?)
assert(obj.one?, 'message')

# good
assert_predicate(obj, :one?)
assert_predicate(obj, :one?, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.
