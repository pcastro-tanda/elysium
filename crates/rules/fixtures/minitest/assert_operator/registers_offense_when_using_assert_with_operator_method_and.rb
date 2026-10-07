class FooTest < Minitest::Test
  def test_do_something
    assert(expected < actual, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_operator(expected, :<, actual, 'message')`.
  end
end
