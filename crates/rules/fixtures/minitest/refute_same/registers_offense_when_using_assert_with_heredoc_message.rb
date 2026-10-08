class FooTest < Minitest::Test
  def test_do_something
    refute(expected.equal?(actual), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_same(expected, actual, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
