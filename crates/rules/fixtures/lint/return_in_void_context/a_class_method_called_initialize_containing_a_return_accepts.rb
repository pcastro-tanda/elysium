class A
  def self.initialize
    foo
    return :qux if bar?
    foo
  end
end
