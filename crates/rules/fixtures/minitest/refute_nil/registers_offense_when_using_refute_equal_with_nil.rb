class FooTest < Minitest::Test
  def test_do_something
    refute_equal(nil, somestuff)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_nil(somestuff)`.
  end
end
