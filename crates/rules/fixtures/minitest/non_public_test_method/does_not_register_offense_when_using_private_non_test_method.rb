class FooTest < Minitest::Test
  private
  def does_something
    assert_equal 42, do_something
  end
end
