class FooTest < Minitest::Test
  def test_do_something
    assert(matcher.=~(object))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_match(matcher, object)`.
  end
end
