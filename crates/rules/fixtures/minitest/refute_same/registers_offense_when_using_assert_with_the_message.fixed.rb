class FooTest < Minitest::Test
  def test_do_something
    refute_same(expected, actual, 'message')
  end
end
