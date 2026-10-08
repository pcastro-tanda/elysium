class FooTest < Minitest::Test
  def test_do_something
    refute_match(/regexp/, object)
  end
end
