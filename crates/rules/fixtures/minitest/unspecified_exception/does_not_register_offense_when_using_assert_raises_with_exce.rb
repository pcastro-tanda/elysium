class FooTest < Minitest::Test
  def test_do_something
    assert_raises(FooException) { raise FooException }
  end
end
