class FooTest < Minitest::Test
  def test_do_something
    refute_nil(object, 'message')
  end
end
