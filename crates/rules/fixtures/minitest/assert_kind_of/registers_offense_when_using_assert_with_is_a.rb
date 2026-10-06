class FooTest < Minitest::Test
  def test_do_something
    assert(object.is_a?(SomeClass))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_kind_of(SomeClass, object)`.
  end
end
