class FooTest < Minitest::Test
  def test_do_something
    assert File.exist?(path)
    ^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_path_exists path`.
  end
end
