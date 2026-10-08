class FooTest < Minitest::Test
  def test_do_something
    assert_includes(collection, object, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
