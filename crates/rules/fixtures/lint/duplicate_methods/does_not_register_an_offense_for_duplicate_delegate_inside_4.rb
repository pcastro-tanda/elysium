A = Module.new do
  def some_method
    implement 1
  end

  if cond
    delegate :some_method, to: :foo
  end
end
