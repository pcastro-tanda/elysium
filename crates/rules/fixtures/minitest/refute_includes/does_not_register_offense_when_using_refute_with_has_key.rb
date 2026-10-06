class FooTest < Minitest::Test
  def test_do_something
    refute(collection.has_key?(object))
  end
end
