class FooTest < Minitest::Test
  def test_do_something
    refute_equal(nil, obj.do_something, <<~MESSAGE
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_nil(obj.do_something, <<~MESSAGE)`.
      message
    MESSAGE
    )
  end
end
