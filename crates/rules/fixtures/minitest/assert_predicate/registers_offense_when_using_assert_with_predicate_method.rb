class FooTest < Minitest::Test
  def test_do_something
    assert(obj.one?)
    ^^^^^^^^^^^^^^^^ Prefer using `assert_predicate(obj, :one?)`.
  end
end
