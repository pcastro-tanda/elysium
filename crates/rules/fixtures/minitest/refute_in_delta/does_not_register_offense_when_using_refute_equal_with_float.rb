class FooTest < Minitest::Test
  def test_do_something
    refute_equal foo, bar
  end
end
