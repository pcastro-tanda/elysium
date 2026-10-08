class FooTest < Minitest::Test
  def test_do_something
    refute(object.instance_of?(SomeClass), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_instance_of(SomeClass, object, 'message')`.
  end
end
