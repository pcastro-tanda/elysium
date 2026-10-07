class FooTest < Minitest::Test
  def test_do_something
    refute_equal(foo, 0.2)
    ^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_in_delta(foo, 0.2)`.
  end
end
