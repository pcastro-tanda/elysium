# Lint/UselessOr

Checks for useless OR expressions.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

Checks for useless OR (`||` and `or`) expressions.

Some methods always return a truthy value, even when called on `nil`
(e.g. `nil.to_i` evaluates to `0`). Therefore, OR expressions appended
after these methods will never evaluate.

@safety
  As shown in the examples below, there are generally two possible ways to
  correct the offense, but this cop's autocorrection always chooses the
  option that preserves the current behavior. While this does not change
  how the code behaves, that option is not necessarily the appropriate fix
  in every situation. For this reason, the autocorrection provided by this
  cop is considered unsafe.

```ruby
# bad
x.to_a || fallback
x.to_s || fallback
x.to_s or fallback

# good - if fallback is same as return value of method called on nil
x.to_a # nil.to_a returns []
x.to_s # nil.to_s returns ''

# good - if the intention is not to call the method on nil
x&.to_a || fallback
x&.to_s || fallback
x&.to_s or fallback
```

## Options

This rule has no options.

## Blind spots

None recorded.
