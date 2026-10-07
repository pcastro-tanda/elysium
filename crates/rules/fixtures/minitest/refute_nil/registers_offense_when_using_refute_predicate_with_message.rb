class FooTest < Minitest::Test
  def test_do_something
    refute_predicate(object, :nil?, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_nil(object, 'message')`.
  end
end
