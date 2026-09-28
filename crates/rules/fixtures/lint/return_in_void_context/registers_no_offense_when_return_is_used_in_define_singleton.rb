class A
  def initialize
    define_singleton_method(:foo) do
      return bar
    end
  end
end
