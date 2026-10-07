class FooTest < Minitest::Test
  def test_do_something
    assert_output("", "") do
    ^^^^^^^^^^^^^^^^^^^^^ Prefer using `assert_silent`.
      puts object.do_something
    end
  end
end
