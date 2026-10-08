class FooTest < Minitest::Test
  def test_do_something
    assert_operator(expected, :<, actual, 'message')
  end
end
