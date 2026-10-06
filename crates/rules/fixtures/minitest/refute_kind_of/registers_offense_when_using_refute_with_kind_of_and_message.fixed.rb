class FooTest < Minitest::Test
  def test_do_something
    refute_kind_of(SomeClass, object, 'message')
  end
end
