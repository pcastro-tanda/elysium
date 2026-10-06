class FooTest < Minitest::Test
  def test_do_something
    assert(obj.expected == other_obj.actual)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_equal(obj.expected, other_obj.actual)`.
  end
end
