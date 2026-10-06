class FooTest < Minitest::Test
  def test_do_something
    assert_equal(false, obj.do_something, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute(obj.do_something, 'message')`.
  end
end
