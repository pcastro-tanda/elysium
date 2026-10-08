class FooTest < Minitest::Test
  def test_do_something
    refute(collection.include?(object), <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_includes(collection, object, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
