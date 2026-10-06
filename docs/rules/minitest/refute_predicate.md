# Minitest/RefutePredicate

This cop enforces the test to use `refute_predicate` instead of using `refute(obj.a_predicate_method?)`.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the test to use `refute_predicate` instead of using `refute(obj.a_predicate_method?)`.

```ruby
# bad
refute(obj.one?)
refute(obj.one?, 'message')

# good
refute_predicate(obj, :one?)
refute_predicate(obj, :one?, 'message')
```

## Options

This rule has no options.

## Blind spots

None recorded.
