class FooTest < Minitest::Test
  def test_do_something
    refute(collection.member?(object))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_includes(collection, object)`.
  end
end
