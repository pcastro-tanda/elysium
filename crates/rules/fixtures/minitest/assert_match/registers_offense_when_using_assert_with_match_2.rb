class FooTest < Minitest::Test
  def test_do_something
    assert(matcher.match?(object))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_match(matcher, object)`.
  end
end
