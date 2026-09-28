class Foo
  def x(obj)
    obj.instance_eval do
      @bar = _1
      def y
      end
    end
  end
end
