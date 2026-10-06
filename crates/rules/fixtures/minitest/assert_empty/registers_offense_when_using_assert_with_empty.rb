class FooTest < Minitest::Test
  def test_do_something
    assert(somestuff.empty?)
    ^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_empty(somestuff)`.
  end
end
