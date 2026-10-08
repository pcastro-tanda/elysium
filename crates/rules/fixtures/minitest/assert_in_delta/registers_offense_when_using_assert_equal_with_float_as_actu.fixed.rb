class FooTest < Minitest::Test
  def test_do_something
    assert_in_delta(foo, 0.2)
  end
end
