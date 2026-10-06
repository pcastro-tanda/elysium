class FooTest < Minitest::Test
  def test_do_something
    assert(File.exist?(path), 'message')
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_path_exists(path, 'message')`.
  end
end
