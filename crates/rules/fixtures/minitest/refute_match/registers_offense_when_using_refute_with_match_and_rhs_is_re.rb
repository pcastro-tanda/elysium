class FooTest < Minitest::Test
  def test_do_something
    refute(object.match(/regexp/))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_match(/regexp/, object)`.
  end
end
