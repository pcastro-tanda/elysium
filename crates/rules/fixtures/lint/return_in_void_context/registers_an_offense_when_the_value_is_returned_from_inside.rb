class A
  def initialize
    proc do
      return :qux
      ^^^^^^ Do not return a value in `initialize`.
    end
  end
end
