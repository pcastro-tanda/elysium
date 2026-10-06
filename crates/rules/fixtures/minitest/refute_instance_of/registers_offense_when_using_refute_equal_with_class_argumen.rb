class FooTest < Minitest::Test
  def test_do_something
    refute_equal(SomeClass, obj.class)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_instance_of(SomeClass, obj)`.
  end
end
