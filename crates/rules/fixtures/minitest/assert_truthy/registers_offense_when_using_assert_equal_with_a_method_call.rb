class FooTest < Minitest::Test
  def test_do_something
    assert_equal(true, obj.is_something?, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert(obj.is_something?, 'message')`.
  end
end
