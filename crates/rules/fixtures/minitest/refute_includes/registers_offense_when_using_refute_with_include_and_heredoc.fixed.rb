class FooTest < Minitest::Test
  def test_do_something
    refute_includes(collection, object, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
