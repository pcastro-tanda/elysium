# Minitest/UselessAssertion

Detects useless assertions (assertions that either always pass or always fail).

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Detects useless assertions (assertions that either always pass or always fail).

```ruby
# bad
assert true
assert_equal @foo, @foo
assert_nil [foo, bar]

# good
assert something
assert_equal foo, bar
assert_nil foo
assert false, "My message"
```

## Options

This rule has no options.

## Blind spots

None recorded.
