class FooTest < Minitest::Test
  def test_do_something
    assert(expected.eql?(actual))
    assert(foo.combines?(bar))
  end
end
