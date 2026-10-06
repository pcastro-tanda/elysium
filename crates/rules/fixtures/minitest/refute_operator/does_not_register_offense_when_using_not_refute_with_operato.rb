class FooTest < Minitest::Test
  def test_do_something
    refute_operator(expected, :<, actual)
  end
end
