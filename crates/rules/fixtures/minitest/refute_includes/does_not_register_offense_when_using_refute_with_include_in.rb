class FooTest < Minitest::Test
  def test_do_something
    refute((collection.include?(object)))
  end
end
