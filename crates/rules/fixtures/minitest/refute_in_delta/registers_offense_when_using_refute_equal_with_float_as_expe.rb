class FooTest < Minitest::Test
  def test_do_something
    refute_equal(0.2, foo)
    ^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_in_delta(0.2, foo)`.
  end
end
