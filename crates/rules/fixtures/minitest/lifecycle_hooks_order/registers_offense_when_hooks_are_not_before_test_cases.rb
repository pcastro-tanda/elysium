class FooTest < Minitest::Test
  def test_something
    assert_equal foo, bar
  end

  def setup; end
  ^^^^^^^^^^^^^^ `setup` is supposed to appear before `test_something`.
  def teardown; end
end
