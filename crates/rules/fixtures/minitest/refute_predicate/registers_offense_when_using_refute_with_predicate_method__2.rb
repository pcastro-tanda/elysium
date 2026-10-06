class FooTest < Minitest::Test
  def test_do_something
    refute(obj.one?, <<~MESSAGE)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_predicate(obj, :one?, <<~MESSAGE)`.
      message
    MESSAGE
  end
end
