class FooTest < Minitest::Test
  def test_do_something
    assert_predicate(obj, :one?, 'message')
  end
end
