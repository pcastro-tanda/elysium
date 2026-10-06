class FooTest < Minitest::Test
  def test_do_something
    assert_operator('rubocop-minitest', :==, actual)
  end
end
