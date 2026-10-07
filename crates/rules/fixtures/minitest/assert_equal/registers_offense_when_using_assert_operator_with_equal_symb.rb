class FooTest < Minitest::Test
  def test_do_something
    assert_operator('rubocop-minitest', :==, actual)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_equal('rubocop-minitest', actual)`.
  end
end
