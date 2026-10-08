class FooTest < Minitest::Test
  def test_do_something
    assert_same(obj.expected, other_obj.actual)
  end
end
