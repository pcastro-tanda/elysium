class A
  def initialize
    return :qux if bar?
    ^^^^^^ Do not return a value in `initialize`.
  end
end
