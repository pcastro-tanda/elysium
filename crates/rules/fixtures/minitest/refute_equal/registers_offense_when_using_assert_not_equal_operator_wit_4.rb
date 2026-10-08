class FooTest < Minitest::Test
  def test_do_something
    assert('rubocop-minitest' != actual, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_equal('rubocop-minitest', actual, 'message')`.
  end
end
