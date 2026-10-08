class FooTest < Minitest::Test
  def test_do_something
    assert_respond_to(object, :do_something)
  end
end
