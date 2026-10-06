class FooTest < Minitest::Test
  def test_do_something
    $stdout = StringIO.new
    assert_equal foo, bar
  end
end
