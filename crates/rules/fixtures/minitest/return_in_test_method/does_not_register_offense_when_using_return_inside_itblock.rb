class FooTest < Minitest::Test
  def test_foo
    Foo.class_eval do
      it.extend(ClassMethods)
      def foo
        return 100
      end
    end
    assert_equal 100, Foo.new.foo
  end
end
