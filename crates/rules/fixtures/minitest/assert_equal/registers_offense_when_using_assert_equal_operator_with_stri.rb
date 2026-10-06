class FooTest < Minitest::Test
  def test_do_something
    assert('rubocop-minitest' == actual)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_equal('rubocop-minitest', actual)`.
  end
end
