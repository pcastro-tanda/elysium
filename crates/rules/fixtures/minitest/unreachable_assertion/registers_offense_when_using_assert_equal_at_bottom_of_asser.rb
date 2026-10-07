assert_raises FooError do
  obj.foo
  assert_equal('foo', obj.bar)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Unreachable `assert_equal` detected.
end
