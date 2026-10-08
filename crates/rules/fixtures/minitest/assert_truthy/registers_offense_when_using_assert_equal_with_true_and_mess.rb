class FooTest < Minitest::Test
  def test_do_something
    assert_equal(true, somestuff, 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert(somestuff, 'message')`.
  end
end
