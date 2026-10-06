class FooTest < Minitest::Test
  def test_do_something
    refute(expected.eql?(actual))
    refute(foo.combines?(bar))
  end
end
