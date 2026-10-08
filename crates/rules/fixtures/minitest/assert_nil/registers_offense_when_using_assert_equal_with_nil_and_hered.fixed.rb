class FooTest < Minitest::Test
  def test_do_something
    assert_nil(obj.do_something, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
