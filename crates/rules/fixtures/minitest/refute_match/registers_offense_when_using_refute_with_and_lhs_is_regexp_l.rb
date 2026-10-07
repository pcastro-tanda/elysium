class FooTest < Minitest::Test
  def test_do_something
    refute(/regexp/.=~(object))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_match(/regexp/, object)`.
  end
end
