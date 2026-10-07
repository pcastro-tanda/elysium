class FooTest < Minitest::Test
  def test_do_something
    assert_equal(foo, 0.2)
    ^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_in_delta(foo, 0.2)`.
  end
end
