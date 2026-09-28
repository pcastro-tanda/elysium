class A
  def initialize
    define_method(:foo) do
      return bar
    end
  end
end
