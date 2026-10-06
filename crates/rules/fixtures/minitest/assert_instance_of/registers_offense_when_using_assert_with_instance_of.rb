class FooTest < Minitest::Test
  def test_do_something
    assert(object.instance_of?(SomeClass))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_instance_of(SomeClass, object)`.
  end
end
