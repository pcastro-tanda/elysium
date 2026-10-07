class FooTest < Minitest::Test
  def test_do_something
    assert(!test)
    ^^^^^^^^^^^^^ Prefer using `refute(test)`.
  end
end
