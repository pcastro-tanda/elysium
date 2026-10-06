class FooTest < Minitest::Test
  def test_do_something
    var = do_something

    refute(var)
  end
end
