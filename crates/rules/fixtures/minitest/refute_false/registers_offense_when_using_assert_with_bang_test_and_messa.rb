class FooTest < Minitest::Test
  def test_do_something
    assert(!test, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute(test, 'message')`.
  end
end
