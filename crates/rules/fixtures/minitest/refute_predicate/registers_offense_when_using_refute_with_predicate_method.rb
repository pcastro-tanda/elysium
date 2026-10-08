class FooTest < Minitest::Test
  def test_do_something
    refute(obj.one?)
    ^^^^^^^^^^^^^^^^ Prefer using `refute_predicate(obj, :one?)`.
  end
end
