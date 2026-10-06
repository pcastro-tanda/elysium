class FooTest < Minitest::Test
  def test_do_something
    refute(expected.equal?(actual))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_same(expected, actual)`.
  end
end
