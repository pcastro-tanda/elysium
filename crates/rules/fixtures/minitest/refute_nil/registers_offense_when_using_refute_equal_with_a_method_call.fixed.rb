class FooTest < Minitest::Test
  def test_do_something
    refute_nil(obj.do_something, 'message')
  end
end
