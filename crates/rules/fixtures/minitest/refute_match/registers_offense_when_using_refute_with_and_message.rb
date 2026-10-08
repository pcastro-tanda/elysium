class FooTest < Minitest::Test
  def test_do_something
    refute(matcher.=~(object), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_match(matcher, object, 'message')`.
  end
end
