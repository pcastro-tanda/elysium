class Foo
  def x(obj)
    obj.instance_eval do
      def y
      end
    end
  end
end
