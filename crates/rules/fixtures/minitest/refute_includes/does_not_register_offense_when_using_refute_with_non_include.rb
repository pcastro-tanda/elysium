class FooTest < Minitest::Test
  def test_do_something
    refute(collection.end_with?(string))
  end
end
