class FooTest < Minitest::Test
  def test_do_something
    refute((matcher.=~(string)))
  end
end
