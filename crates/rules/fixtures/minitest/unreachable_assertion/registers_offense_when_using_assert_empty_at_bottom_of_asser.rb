assert_raises FooError do
  obj.foo
  assert_empty(obj.bar)
  ^^^^^^^^^^^^^^^^^^^^^ Unreachable `assert_empty` detected.
end
