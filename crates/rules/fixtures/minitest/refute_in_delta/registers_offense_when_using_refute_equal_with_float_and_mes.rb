class FooTest < Minitest::Test
  def test_do_something
    refute_equal(0.2, foo, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_in_delta(0.2, foo, 0.001, 'message')`.
  end
end
