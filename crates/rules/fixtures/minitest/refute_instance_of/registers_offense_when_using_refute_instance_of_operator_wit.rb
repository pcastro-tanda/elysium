class FooTest < Minitest::Test
  def test_do_something
    refute(object.instance_of?(SomeClass), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_instance_of(SomeClass, object, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
