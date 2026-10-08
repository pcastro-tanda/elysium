class FooTest < Minitest::Test
  def test_do_something
    assert_equal(false, obj.do_something, <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute(obj.do_something, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
