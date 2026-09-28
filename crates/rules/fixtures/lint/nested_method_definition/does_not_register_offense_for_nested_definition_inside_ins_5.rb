class Foo
  def x(obj)
    obj.instance_eval do
      @bar = it
      def y
      end
    end
  end
end
