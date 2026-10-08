class FooTest < Minitest::Test
  def test_do_something
    refute(matcher.match?(object))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_match(matcher, object)`.
  end
end
