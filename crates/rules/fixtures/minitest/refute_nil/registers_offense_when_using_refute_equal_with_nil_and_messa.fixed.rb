class FooTest < Minitest::Test
  def test_do_something
    refute_nil(somestuff, 'message')
  end
end
