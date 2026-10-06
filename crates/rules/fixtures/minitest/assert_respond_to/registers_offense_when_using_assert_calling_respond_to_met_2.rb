class FooTest < Minitest::Test
  def test_do_something
    assert(object.respond_to?(:do_something), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_respond_to(object, :do_something, 'message')`.
  end
end
