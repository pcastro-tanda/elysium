class ParentTest < Minitest::Test
end

class ChildTest < ParentTest
  def test_child_asserts_twice
    assert_equal(1, 1)
  end
end
