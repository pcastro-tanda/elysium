class FooTest < Minitest::Test
  def test_do_something
    assert(obj.one?, <<~MESSAGE)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_predicate(obj, :one?, <<~MESSAGE)`.
      message
    MESSAGE
  end
end
