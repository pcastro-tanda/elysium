class FooTest < Minitest::Test
  def test_do_something
    refute_predicate(obj, :one?)
  end
end
