class FooTest < Minitest::Test
  def test_do_something
    refute_same(obj.expected, other_obj.actual)
  end
end
