class FooTest < Minitest::Test
  def test_do_something
    refute_operator(matcher, :=~, object)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_match(matcher, object)`.
  end
end
