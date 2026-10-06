class FooTest < Minitest::Test
  def test_do_something
    assert(File.empty?(path))
  end
end
