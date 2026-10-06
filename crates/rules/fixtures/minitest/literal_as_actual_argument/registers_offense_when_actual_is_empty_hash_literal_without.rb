class FooTest < Minitest::Test
  def test_do_something
    assert_equal foo, {}
                 ^^^^^^^ Replace the literal with the first argument.
  end
end
