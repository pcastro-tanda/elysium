class FooTest < Minitest::Test
  def test_do_something
    assert_equal(SomeClass, obj.class, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_instance_of(SomeClass, obj, 'message')`.
  end
end
