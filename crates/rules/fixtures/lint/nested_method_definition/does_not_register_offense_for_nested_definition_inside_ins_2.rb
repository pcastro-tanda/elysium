class Foo
  def x(obj)
    obj.instance_exec do
      def y
      end
    end
  end
end
