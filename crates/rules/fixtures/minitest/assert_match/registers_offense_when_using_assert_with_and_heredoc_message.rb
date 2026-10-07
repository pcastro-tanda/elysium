class FooTest < Minitest::Test
  def test_do_something
    assert(matcher.=~(object), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_match(matcher, object, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
