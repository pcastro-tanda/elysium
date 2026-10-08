class FooTest < Minitest::Test
  def test_do_something
    refute((somestuff.empty?))
  end
end
