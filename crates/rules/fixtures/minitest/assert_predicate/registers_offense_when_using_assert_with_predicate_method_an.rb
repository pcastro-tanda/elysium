class FooTest < Minitest::Test
  def test_do_something
    assert(obj.one?, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_predicate(obj, :one?, 'message')`.
  end
end
