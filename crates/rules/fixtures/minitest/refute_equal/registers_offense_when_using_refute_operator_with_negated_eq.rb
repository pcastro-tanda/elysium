class FooTest < Minitest::Test
  def test_do_something
    refute_operator('rubocop-minitest', :!=, actual)
  end
end
