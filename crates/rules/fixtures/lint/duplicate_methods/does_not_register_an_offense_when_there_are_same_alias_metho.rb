module FooTest
  def make_save_always_fail
    Foo.class_eval do
      def failed_save
        raise
      end
      alias_method :original_save, :save
      alias_method :save, :failed_save
    end

    yield
  ensure
    Foo.class_eval do
      alias_method :save, :original_save
    end
  end
end
