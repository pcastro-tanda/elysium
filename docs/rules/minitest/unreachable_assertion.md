# Minitest/UnreachableAssertion

This cop checks for an `assert_raises` block containing any unreachable assertions.

| | |
| --- | --- |
| Department | Minitest |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for `assert_raises` has an assertion method at the bottom of block because the assertion will be never reached.

```ruby
# bad
assert_raises FooError do
  obj.occur_error
  assert_equal('foo', obj.bar) # Never asserted.
end

# good
assert_raises FooError do
  obj.occur_error
end
assert_equal('foo', obj.bar)
```

## Options

This rule has no options.

## Blind spots

None recorded.
