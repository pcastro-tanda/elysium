class FooTest < Minitest::Test
  def test_do_something
    refute(collection.key?(object))
  end
end
