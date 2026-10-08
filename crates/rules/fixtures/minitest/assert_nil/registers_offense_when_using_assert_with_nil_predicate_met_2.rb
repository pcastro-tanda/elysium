class FooTest < Minitest::Test
  def test_do_something
    assert(somestuff.nil?, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_nil(somestuff, 'message')`.
  end
end
