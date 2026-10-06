class FooTest < Minitest::Test
  def test_do_something
    assert(collection.include?(object), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_includes(collection, object, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
