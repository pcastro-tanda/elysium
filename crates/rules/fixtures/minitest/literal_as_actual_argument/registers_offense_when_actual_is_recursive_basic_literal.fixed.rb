class FooTest < Minitest::Test
  def test_do_something
    assert_equal([1, 2, { key: :value }], foo)
  end
end
