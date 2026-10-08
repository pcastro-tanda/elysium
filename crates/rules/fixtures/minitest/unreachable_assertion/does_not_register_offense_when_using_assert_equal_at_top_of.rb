assert_raises FooError do
  assert_equal('foo', obj.bar)
  obj.foo
end
