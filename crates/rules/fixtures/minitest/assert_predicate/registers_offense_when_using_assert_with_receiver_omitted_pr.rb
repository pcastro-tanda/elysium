class FooTest < Minitest::Test
  def test_do_something
    assert(one?)
    ^^^^^^^^^^^^ Prefer using `assert_predicate(self, :one?)`.
  end
end
