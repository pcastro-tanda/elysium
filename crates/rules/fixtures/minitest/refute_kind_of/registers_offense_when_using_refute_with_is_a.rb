class FooTest < Minitest::Test
  def test_do_something
    refute(object.is_a?(SomeClass))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_kind_of(SomeClass, object)`.
  end
end
