class FooTest < Minitest::Test
  def test_do_something
    assert(respond_to?(:do_something))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_respond_to(self, :do_something)`.
  end
end
