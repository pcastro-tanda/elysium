class FooTest < Minitest::Test
  def test_do_something
    refute(somestuff.empty?)
    ^^^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_empty(somestuff)`.
  end
end
