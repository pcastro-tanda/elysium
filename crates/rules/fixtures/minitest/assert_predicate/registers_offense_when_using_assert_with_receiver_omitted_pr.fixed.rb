class FooTest < Minitest::Test
  def test_do_something
    assert_predicate(self, :one?)
  end
end
