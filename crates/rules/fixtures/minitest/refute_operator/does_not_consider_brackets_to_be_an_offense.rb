class FooTest < Minitest::Test
  def test_do_something
    refute(array_of_booleans[42])
  end
end
