class FooTest < Minitest::Test
  def test_do_something
    refute(somestuff.nil?, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_nil(somestuff, 'message')`.
  end
end
