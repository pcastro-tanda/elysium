class FooTest < Minitest::Test
  def test_do_something
    assert_operator('rubocop-minitest', :==, actual, message)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_equal('rubocop-minitest', actual, message)`.
  end
end
