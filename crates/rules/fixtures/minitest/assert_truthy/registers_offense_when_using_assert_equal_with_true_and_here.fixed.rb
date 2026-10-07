class FooTest < Minitest::Test
  def test_do_something
    assert(obj.is_something?, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
