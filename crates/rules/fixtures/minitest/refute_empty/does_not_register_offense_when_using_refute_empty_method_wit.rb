class FooTest < Minitest::Test
  def test_do_something
    refute(File.empty?(path))
  end
end
