class A
  def initialize
    lambda do
      return :qux
    end
  end
end
