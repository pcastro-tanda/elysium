class FooTest < Minitest::Test
  def test_do_something
    refute((matcher.match?(string)))
  end
end
