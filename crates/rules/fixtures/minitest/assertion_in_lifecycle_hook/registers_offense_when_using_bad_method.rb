class FooTest < Minitest::Test
  def setup
    assert_equal(foo, bar)
    ^^^^^^^^^^^^^^^^^^^^^^ Do not use `assert_equal` in `setup` hook.
  end
end
