class FooTest < Minitest::Test
  def test_do_something
    assert_equal(expected.object_id, actual.object_id)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_same(expected, actual)`.
  end
end
