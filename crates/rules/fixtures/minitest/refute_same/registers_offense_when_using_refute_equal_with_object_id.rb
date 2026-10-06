class FooTest < Minitest::Test
  def test_do_something
    refute_equal(expected.object_id, actual.object_id)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_same(expected, actual)`.
  end
end
