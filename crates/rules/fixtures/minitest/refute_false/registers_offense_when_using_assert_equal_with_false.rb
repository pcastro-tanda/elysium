class FooTest < Minitest::Test
  def test_do_something
    assert_equal(false, somestuff)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute(somestuff)`.
  end
end
