class FooTest < Minitest::Test
  def test_do_something
    assert(collection.include?(object), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_includes(collection, object, 'message')`.
  end
end
