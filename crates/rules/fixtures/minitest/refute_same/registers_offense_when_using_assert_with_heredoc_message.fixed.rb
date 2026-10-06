class FooTest < Minitest::Test
  def test_do_something
    refute_same(expected, actual, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
