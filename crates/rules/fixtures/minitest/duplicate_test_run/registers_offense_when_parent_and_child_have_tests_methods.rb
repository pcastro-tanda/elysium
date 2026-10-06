class ParentTest < Minitest::Test
  def test_parent
  end
end

class ChildTest < ParentTest
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Subclasses with test methods causes the parent' tests to run them twice.
  def test_child_asserts_twice
    assert_equal(1, 1)
  end
end
