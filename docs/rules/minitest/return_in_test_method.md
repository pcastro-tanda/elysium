# Minitest/ReturnInTestMethod

Enforces the use of `skip` instead of `return` in test methods.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of `skip` instead of `return` in test methods.

```ruby
# bad
def test_something
  return if condition?
  assert_equal(42, something)
end

# good
def test_something
  skip if condition?
  assert_equal(42, something)
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
