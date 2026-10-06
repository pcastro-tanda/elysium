class FooTest < Minitest::Test
  def test_do_something
    refute(obj.do_something, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
