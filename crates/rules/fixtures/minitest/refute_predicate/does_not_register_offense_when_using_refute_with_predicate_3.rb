class FooTest < Minitest::Test
  def test_do_something
    refute([1, 2, 3].any? { some_filter_function _1 })
  end
end
