class FooTest < Minitest::Test
  def test_do_something
    refute(object.=~(/regexp/))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_match(/regexp/, object)`.
  end
end
