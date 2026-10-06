class FooTest < Minitest::Test
  def test_do_something
    assert_operator(matcher, :!~, object)
  end
end
