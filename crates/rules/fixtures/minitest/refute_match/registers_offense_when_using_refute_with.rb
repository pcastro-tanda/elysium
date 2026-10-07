class FooTest < Minitest::Test
  def test_do_something
    refute(matcher.=~(object))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_match(matcher, object)`.
  end
end
