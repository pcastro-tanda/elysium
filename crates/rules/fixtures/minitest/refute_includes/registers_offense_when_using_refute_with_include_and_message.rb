class FooTest < Minitest::Test
  def test_do_something
    refute(collection.include?(object), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_includes(collection, object, 'message')`.
  end
end
