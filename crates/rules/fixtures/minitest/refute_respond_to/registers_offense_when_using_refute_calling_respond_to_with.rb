class FooTest < Minitest::Test
  def test_do_something
    refute(respond_to?(:do_something))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_respond_to(self, :do_something)`.
  end
end
