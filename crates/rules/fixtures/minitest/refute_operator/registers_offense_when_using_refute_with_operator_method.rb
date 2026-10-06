class FooTest < Minitest::Test
  def test_do_something
    refute(expected < actual)
    ^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_operator(expected, :<, actual)`.
  end
end
