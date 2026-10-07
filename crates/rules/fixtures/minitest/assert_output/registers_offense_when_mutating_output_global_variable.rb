class FooTest < Minitest::Test
  def test_do_something
    $stdout = StringIO.new
    puts object.method
    $stdout.rewind
    assert_match expected, $stdout.read
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `assert_output` instead of mutating $stdout.
  end
end
