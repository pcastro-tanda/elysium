class FooTest < Minitest::Test
  def test_do_something
    assert_path_exists path, 'message'
  end
end
