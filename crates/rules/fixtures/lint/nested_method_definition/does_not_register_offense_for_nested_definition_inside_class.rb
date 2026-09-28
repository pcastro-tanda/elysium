class Foo
  def x(klass)
    klass.class_eval do
      def y
      end
    end
  end
end
