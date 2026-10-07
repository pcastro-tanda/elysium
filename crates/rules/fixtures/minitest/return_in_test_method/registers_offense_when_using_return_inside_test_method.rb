class FooTest < Minitest::Test
  def test_foo
    return if something?
    ^^^^^^ Use `skip` instead of `return`.
    assert_equal foo, bar
  end
end
