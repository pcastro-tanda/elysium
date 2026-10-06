module M
  class FooTest < Minitest::Test
    def test_foo
    end
  end

  def test_bar
  ^^^^^^^^^^^^ Test method should be defined inside a test class to ensure execution.
  end
end
