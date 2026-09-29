class SomeClass
  def attr(*args)
    p args
  end

  def a
    attr(1)
  end
end
