class FooTest < Minitest::Test
  def test_do_something
    assert_instance_of(SomeClass, object, 'message')
  end
end
