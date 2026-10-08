class FooTest < Minitest::Test
  def test_do_something
    assert(object.instance_of?(SomeClass), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_instance_of(SomeClass, object, 'message')`.
  end
end
