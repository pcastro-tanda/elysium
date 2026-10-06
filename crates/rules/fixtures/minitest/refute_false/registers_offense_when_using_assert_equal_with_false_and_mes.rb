class FooTest < Minitest::Test
  def test_do_something
    assert_equal(false, somestuff, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute(somestuff, 'message')`.
  end
end
