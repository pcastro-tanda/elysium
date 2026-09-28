class Foo
  def self.define
    Struct.new(:name) do
      def y
      end
    end
  end
end

class Foo
  def self.define
    Struct.new do
      def y
      end
    end
  end
end
