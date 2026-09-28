class A
  def initialize
    foo do
      it
      return :qux
      ^^^^^^ Do not return a value in `initialize`.
    end
  end
end
