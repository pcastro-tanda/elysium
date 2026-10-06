class FooTest < Minitest::Test
  def test_do_something
    refute(matcher.=~(object), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_match(matcher, object, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
