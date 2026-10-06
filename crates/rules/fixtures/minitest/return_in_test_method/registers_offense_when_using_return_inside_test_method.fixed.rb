class FooTest < Minitest::Test
  def test_foo
    skip if something?
    assert_equal foo, bar
  end
end
