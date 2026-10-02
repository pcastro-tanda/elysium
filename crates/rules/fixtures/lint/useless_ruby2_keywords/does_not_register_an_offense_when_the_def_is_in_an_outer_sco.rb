class Outer
  def foo(**kwargs)
  end

  class Inner
    ruby2_keywords :foo
  end
end
