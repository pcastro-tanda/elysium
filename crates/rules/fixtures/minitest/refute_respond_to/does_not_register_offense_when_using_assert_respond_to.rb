class FooTest < Minitest::Test
  def test_do_something
    refute_respond_to('rubocop-minitest', :do_something)
  end
end
