class FooTest < Minitest::Test
  def test_do_something
    assert(somestuff.nil?)
    ^^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_nil(somestuff)`.
  end
end
