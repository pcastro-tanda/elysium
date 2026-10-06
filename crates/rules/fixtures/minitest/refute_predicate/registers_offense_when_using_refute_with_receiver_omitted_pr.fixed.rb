class FooTest < Minitest::Test
  def test_do_something
    refute_predicate(self, :one?)
  end
end
