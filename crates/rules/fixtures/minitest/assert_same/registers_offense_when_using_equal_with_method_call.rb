class FooTest < Minitest::Test
  def test_do_something
    assert(obj.expected.equal?(other_obj.actual))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_same(obj.expected, other_obj.actual)`.
  end
end
