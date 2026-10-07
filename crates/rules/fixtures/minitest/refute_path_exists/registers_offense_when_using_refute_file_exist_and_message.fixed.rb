class FooTest < Minitest::Test
  def test_do_something
    refute_path_exists(path, 'message')
  end
end
