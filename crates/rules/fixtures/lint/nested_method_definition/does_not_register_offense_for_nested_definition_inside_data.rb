class Foo
  def self.define
    Data.define(:name) do
      def y
      end
    end
  end
end

class Foo
  def self.define
    Data.define do
      def y
      end
    end
  end
end
