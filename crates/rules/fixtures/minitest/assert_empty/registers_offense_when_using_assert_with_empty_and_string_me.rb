class FooTest < Minitest::Test
  def test_do_something
    assert(somestuff.empty?, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_empty(somestuff, 'message')`.
  end
end
