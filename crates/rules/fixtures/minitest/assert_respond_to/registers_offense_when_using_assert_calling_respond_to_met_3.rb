class FooTest < Minitest::Test
  def test_do_something
    assert(respond_to?(:do_something), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_respond_to(self, :do_something, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
