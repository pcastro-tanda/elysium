class FooTest < Minitest::Test
  def test_do_something
    refute(obj.one?, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_predicate(obj, :one?, 'message')`.
  end
end
