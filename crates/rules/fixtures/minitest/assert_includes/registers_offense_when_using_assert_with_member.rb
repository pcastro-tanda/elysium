class FooTest < Minitest::Test
  def test_do_something
    assert(collection.member?(object))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_includes(collection, object)`.
  end
end
