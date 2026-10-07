class FooTest < Minitest::Test
  def test_do_something
    assert_output("something", "") do
      puts object.do_something
    end
  end
end
