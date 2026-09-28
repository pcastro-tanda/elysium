class A
  def bar
    foo
    return :qux if bar?
    foo
  end
end
