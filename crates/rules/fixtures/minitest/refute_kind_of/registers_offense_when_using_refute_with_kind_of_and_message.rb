class FooTest < Minitest::Test
  def test_do_something
    refute(object.is_a?(SomeClass), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_kind_of(SomeClass, object, 'message')`.
  end
end
