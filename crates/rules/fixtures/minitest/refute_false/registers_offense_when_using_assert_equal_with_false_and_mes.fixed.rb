class FooTest < Minitest::Test
  def test_do_something
    refute(somestuff, 'message')
  end
end
