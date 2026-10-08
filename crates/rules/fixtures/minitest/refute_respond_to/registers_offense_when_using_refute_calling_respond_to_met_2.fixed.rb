class FooTest < Minitest::Test
  def test_do_something
    refute_respond_to(object, :do_something, 'message')
  end
end
