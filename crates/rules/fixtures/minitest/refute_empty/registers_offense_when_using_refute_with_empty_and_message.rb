class FooTest < Minitest::Test
  def test_do_something
    refute(somestuff.empty?, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_empty(somestuff, 'message')`.
  end
end
