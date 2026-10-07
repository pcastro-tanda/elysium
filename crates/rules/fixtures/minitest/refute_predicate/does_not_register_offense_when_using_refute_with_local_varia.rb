class FooTest < Minitest::Test
  def test_do_something
    obj = create_obj
    refute(obj)
  end
end
