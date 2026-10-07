class FooTest < Minitest::Test
  def test_do_something
    refute_equal(SomeClass, obj.class, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_instance_of(SomeClass, obj, 'message')`.
  end
end
