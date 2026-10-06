class FooTest < Minitest::Test
  def test_do_something
    assert_equal(nil, obj.do_something, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_nil(obj.do_something, 'message')`.
  end
end
