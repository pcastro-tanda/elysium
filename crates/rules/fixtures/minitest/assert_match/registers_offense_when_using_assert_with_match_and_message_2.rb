class FooTest < Minitest::Test
  def test_do_something
    assert(matcher.match(object), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_match(matcher, object, 'message')`.
  end
end
