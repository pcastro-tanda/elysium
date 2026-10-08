class FooTest < Minitest::Test
  def test_do_something
    assert_nil(obj.do_something, 'message')
  end
end
