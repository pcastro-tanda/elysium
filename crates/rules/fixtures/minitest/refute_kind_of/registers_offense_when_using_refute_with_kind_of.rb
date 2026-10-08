class FooTest < Minitest::Test
  def test_do_something
    refute(object.kind_of?(SomeClass))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_kind_of(SomeClass, object)`.
  end
end
