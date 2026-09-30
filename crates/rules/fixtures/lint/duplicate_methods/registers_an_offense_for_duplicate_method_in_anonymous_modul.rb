Module.new do
  def some_method
    implement 1
  end
  def some_method
  ^^^^^^^^^^^^^^^ Method `Object#some_method` is defined at both (string):2 and (string):5.
    implement 2
  end
end
