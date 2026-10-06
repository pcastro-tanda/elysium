class FooTest < Minitest::Test
  def test_do_something
    refute_predicate(object, :nil?)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_nil(object)`.
  end
end
