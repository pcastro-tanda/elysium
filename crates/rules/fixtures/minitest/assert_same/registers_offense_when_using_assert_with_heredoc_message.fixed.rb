class FooTest < Minitest::Test
  def test_do_something
    assert_same(expected, actual, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
