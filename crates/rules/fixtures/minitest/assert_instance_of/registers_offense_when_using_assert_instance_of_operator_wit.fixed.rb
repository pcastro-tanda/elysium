class FooTest < Minitest::Test
  def test_do_something
    assert_instance_of(SomeClass, object, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
