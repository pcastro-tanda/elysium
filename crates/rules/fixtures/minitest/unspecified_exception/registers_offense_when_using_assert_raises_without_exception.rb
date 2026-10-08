class FooTest < Minitest::Test
  def test_do_something
    assert_raises { raise FooException }
    ^^^^^^^^^^^^^ Specify the exception being captured.
  end
end
