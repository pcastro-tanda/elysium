class FooTest < Minitest::Test
  def test_do_something
    assert(!'rubocop-minitest' == object, 'message')
  end
end
