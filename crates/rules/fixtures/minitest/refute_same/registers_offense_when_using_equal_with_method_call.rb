class FooTest < Minitest::Test
  def test_do_something
    refute(obj.expected.equal?(other_obj.actual))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_same(obj.expected, other_obj.actual)`.
  end
end
