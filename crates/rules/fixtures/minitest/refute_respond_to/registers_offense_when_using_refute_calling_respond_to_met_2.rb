class FooTest < Minitest::Test
  def test_do_something
    refute(object.respond_to?(:do_something), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_respond_to(object, :do_something, 'message')`.
  end
end
