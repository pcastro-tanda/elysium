class FooTest < Minitest::Test
  def test_do_something
    assert_respond_to(self, :do_something, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
