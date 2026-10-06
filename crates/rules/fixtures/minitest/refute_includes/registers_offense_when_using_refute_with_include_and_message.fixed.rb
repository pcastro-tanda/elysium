class FooTest < Minitest::Test
  def test_do_something
    refute_includes(collection, object, 'message')
  end
end
