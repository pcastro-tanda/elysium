class FooTest < Minitest::Test
  def test_do_something
    assert_match(matcher, object, 'message')
  end
end
