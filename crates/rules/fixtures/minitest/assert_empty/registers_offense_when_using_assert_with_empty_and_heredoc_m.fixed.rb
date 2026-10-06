class FooTest < Minitest::Test
  def test_do_something
    assert_empty(somestuff, <<~MESSAGE
      message
    MESSAGE
    )
  end
end
