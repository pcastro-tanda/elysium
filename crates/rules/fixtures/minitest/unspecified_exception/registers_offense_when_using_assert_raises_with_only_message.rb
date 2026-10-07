class FooTest < Minitest::Test
  def test_do_something
    assert_raises('This should have raised') { raise FooException }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Specify the exception being captured.
  end
end
