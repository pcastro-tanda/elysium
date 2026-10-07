class FooTest < Minitest::Test
  def test_do_something
    refute_equal('rubocop-minitest', actual)
  end
end
