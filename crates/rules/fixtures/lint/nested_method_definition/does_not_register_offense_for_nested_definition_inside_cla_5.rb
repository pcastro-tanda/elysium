class Foo
  def self.define
    ::Class.new(S) do
      def y
      end
    end
  end
end

class Foo
  def self.define
    ::Class.new do
      def y
      end
    end
  end
end
