class FooTest < Minitest::Test
  def test_do_something
    refute_equal(nil, somestuff, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_nil(somestuff, 'message')`.
  end
end
