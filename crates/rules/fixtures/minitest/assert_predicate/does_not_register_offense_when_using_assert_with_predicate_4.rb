class FooTest < Minitest::Test
  def test_do_something
    assert([1, 2, 3].any? { some_filter_function it })
  end
end
