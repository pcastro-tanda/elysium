class A
  def initialize
    foo do
      _1
      return :qux
      ^^^^^^ Do not return a value in `initialize`.
    end
  end
end
