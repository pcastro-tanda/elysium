class FooTest < Minitest::Test
  def test_do_something
    refute_match(matcher, object)
  end
end
