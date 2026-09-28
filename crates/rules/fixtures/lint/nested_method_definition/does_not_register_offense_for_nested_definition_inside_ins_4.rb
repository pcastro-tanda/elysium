class Foo
  def x(obj)
    obj.instance_exec(3) do
      @bar = _1
      def y
      end
    end
  end
end
