class FooTest < Minitest::Test
  def test_do_something
    refute(expected < actual, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_operator(expected, :<, actual, 'message')`.
  end
end
