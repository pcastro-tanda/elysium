class A
  redefine_method(:some_method) do
    implement 2
  end

  def some_method
    implement 3
  end
end
