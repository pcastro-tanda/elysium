class FooTest < Minitest::Test
  def test_do_something
    refute_equal(nil, obj.do_something, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_nil(obj.do_something, 'message')`.
  end
end
