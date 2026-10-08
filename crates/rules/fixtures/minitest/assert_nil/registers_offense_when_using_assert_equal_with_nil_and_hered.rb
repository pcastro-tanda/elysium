class FooTest < Minitest::Test
  def test_do_something
    assert_equal(nil, obj.do_something, <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_nil(obj.do_something, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
