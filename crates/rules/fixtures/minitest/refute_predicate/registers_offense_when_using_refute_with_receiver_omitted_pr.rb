class FooTest < Minitest::Test
  def test_do_something
    refute(one?)
    ^^^^^^^^^^^^ Prefer using `refute_predicate(self, :one?)`.
  end
end
