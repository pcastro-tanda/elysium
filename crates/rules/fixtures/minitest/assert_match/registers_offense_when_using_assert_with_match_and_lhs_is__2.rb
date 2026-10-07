class FooTest < Minitest::Test
  def test_do_something
    assert(/regexp/.match(object))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_match(/regexp/, object)`.
  end
end
