class FooTest < Minitest::Test
  def test_do_something
    assert_equal(nil, somestuff, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_nil(somestuff, 'message')`.
  end
end
