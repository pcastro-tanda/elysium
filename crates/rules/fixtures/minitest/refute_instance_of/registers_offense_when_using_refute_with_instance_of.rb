class FooTest < Minitest::Test
  def test_do_something
    refute(object.instance_of?(SomeClass))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_instance_of(SomeClass, object)`.
  end
end
