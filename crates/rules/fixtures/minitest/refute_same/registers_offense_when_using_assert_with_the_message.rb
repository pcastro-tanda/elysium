class FooTest < Minitest::Test
  def test_do_something
    refute(expected.equal?(actual), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_same(expected, actual, 'message')`.
  end
end
