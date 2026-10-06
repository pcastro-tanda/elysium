class FooTest < Minitest::Test
  def test_do_something
    assert(expected.equal?(actual), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_same(expected, actual, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
