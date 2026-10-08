class FooTest < Minitest::Test
  def test_do_something
    refute(object.respond_to?(:do_something))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_respond_to(object, :do_something)`.
  end
end
