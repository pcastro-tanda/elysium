class ParentTest < Minitest::Test
  def test_parent
  end
end

class Child1Test < ParentTest
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Subclasses with test methods causes the parent' tests to run them twice.
  def test_parent
  end
end

class Child2Test < ParentTest
end

class Child3Test < ParentTest
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Subclasses with test methods causes the parent' tests to run them twice.
  def test_1_child_2_asserts_twice
  end

  def test_2_child_2_asserts_twice
  end

  def test_3_child_2_asserts_twice
  end
end
