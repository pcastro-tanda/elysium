class FooTest < Minitest::Test
  def test_do_something
    assert(object.instance_of?(SomeClass), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_instance_of(SomeClass, object, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
