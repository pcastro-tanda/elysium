class FooTest < Minitest::Test
  def test_do_something
    assert_kind_of(SomeClass, object, 'message')
  end
end
