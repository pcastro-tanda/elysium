class ParentTest < Minitest::Test
  def test_child_asserts_twice
    assert_equal(1, 1)
  end
end

class ChildTest < ParentTest
end
