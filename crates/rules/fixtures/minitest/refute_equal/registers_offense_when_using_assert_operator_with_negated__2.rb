class FooTest < Minitest::Test
  def test_do_something
    refute_operator('rubocop-minitest', :==, actual, message)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_equal('rubocop-minitest', actual, message)`.
  end
end
