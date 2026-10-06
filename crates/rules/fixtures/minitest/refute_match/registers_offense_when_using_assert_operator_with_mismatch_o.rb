class FooTest < Minitest::Test
  def test_do_something
    assert_operator(matcher, :!~, object)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_match(matcher, object)`.
  end
end
