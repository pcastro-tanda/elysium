class FooTest < Minitest::Test
  def test_do_something
    refute(File.exist?(path), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_path_exists(path, 'message')`.
  end
end
