class FooTest < Minitest::Test
  def test_do_something
    refute_respond_to(self, :do_something)
  end
end
