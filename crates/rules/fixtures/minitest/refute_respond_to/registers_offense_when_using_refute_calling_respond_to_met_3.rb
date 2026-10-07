class FooTest < Minitest::Test
  def test_do_something
    refute(respond_to?(:do_something), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_respond_to(self, :do_something, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
