class FooTest < Minitest::Test
  def test_do_something
    refute_match(matcher, object, 'message')
  end
end
