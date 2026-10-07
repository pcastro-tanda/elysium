class FooTest < Minitest::Test
  def test_do_something
    assert_in_delta(0.2, foo, 0.001, 'message')
  end
end
