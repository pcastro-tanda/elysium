class FooTest < Minitest::Test
  def test_do_something
    refute_instance_of(SomeClass, object, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
