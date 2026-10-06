class FooTest < Minitest::Test
  def test_do_something
    assert_equal(foo, [1, 2, { key: :value }])
                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Replace the literal with the first argument.
  end
end
