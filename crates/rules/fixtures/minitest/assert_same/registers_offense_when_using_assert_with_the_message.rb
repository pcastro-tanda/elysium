class FooTest < Minitest::Test
  def test_do_something
    assert(expected.equal?(actual), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_same(expected, actual, 'message')`.
  end
end
