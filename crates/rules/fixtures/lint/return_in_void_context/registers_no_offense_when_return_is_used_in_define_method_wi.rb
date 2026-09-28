class A
  def initialize
    self.define_method(:foo) do
      return bar
    end
  end
end
