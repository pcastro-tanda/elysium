class FooTest < Minitest::Test
  def test_do_something
    assert_equal(true, somestuff)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert(somestuff)`.
  end
end
