class FooTest < Minitest::Test
  def test_do_something
    assert_predicate(object, :nil?, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_nil(object, 'message')`.
  end
end
