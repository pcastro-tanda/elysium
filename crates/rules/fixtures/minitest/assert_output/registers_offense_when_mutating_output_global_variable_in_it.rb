class FooTest < Minitest::Test
  describe Foo do
    it "does something" do
      $stdout = StringIO.new
      puts object.method
      $stdout.rewind
      assert_match expected, $stdout.read
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `assert_output` instead of mutating $stdout.
    end
  end
end
