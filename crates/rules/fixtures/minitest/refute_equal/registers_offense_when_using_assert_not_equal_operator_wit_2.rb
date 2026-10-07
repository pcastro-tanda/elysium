class FooTest < Minitest::Test
  def test_do_something
    assert(expected != actual)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_equal(expected, actual)`.
  end
end
