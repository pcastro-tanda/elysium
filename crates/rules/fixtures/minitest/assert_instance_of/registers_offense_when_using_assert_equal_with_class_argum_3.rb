class FooTest < Minitest::Test
  def test_do_something
    assert_equal(SomeClass, obj.class, <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_instance_of(SomeClass, obj, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
