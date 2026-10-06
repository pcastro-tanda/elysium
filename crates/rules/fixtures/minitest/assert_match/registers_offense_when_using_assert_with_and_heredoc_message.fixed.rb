class FooTest < Minitest::Test
  def test_do_something
    assert_match(matcher, object, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
