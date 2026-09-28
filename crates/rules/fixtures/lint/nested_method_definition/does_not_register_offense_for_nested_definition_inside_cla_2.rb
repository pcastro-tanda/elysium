class Foo
  def x(klass)
    klass.class_exec do
      def y
      end
    end
  end
end
