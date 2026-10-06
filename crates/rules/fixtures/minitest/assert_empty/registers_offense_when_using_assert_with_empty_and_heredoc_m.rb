class FooTest < Minitest::Test
  def test_do_something
    assert(somestuff.empty?, <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_empty(somestuff, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
