class FooTest < Minitest::Test
  def test_do_something
    refute(somestuff.empty?, <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_empty(somestuff, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
