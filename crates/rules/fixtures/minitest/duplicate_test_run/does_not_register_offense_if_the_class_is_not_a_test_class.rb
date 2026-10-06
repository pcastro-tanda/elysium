class ParentTest < ExampleClass
  def test_child_asserts_twice
    assert_equal(1, 1)
  end
end

class ChildClass < ParentTest
  def test_child_asserts_twice
    assert_equal(1, 1)
  end
end
