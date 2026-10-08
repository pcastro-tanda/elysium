class FooTest < Minitest::Test
  def test_do_something
    refute(somestuff.nil?)
    ^^^^^^^^^^^^^^^^^^^^^^ Prefer using `refute_nil(somestuff)`.
  end
end
