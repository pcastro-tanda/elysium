class FooTest < Minitest::Test
  def test_do_something
    refute File.exist?(path)
    ^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_path_exists path`.
  end
end
