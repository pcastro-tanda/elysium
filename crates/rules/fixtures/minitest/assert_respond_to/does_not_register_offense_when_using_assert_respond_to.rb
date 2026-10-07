class FooTest < Minitest::Test
  def test_do_something
    assert_respond_to('rubocop-minitest', :do_something)
  end
end
