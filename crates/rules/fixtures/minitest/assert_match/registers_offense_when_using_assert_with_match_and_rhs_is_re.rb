class FooTest < Minitest::Test
  def test_do_something
    assert(object.match?(/regexp/))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_match(/regexp/, object)`.
  end
end
