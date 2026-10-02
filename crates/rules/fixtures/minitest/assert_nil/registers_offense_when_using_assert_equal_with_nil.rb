class FooTest < Minitest::Test
  def test_do_something
    assert_equal(nil, somestuff)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_nil(somestuff)`.
  end
end
