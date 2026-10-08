class FooTest < Minitest::Test
  def test_do_something
    assert(!'rubocop-minitest' == actual, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
