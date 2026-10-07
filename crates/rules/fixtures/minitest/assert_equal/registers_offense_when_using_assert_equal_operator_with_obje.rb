class FooTest < Minitest::Test
  def test_do_something
    assert(expected == actual)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_equal(expected, actual)`.
  end
end
