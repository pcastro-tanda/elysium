class FooTest < Minitest::Test
  def test_do_something
    assert_predicate(object, :nil?)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_nil(object)`.
  end
end
