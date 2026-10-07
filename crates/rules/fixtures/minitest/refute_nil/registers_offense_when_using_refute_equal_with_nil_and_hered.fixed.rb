class FooTest < Minitest::Test
  def test_do_something
    refute_nil(obj.do_something, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
