class FooTest < Minitest::Test
  def test_do_something
    assert(expected.equal?(actual))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_same(expected, actual)`.
  end
end
