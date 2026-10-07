class FooTest < Minitest::Test
  def test_do_something
    refute((object.respond_to?(:do_something)))
  end
end
