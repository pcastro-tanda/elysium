class FooTest < Minitest::Test
  def test_do_something
    assert_equal(true, obj.is_something?, <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert(obj.is_something?, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
