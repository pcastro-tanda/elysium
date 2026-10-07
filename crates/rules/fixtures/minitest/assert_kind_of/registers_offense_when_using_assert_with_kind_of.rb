class FooTest < Minitest::Test
  def test_do_something
    assert(object.kind_of?(SomeClass))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_kind_of(SomeClass, object)`.
  end
end
