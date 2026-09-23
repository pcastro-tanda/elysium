class A
  def some_method
    implement 1
  end
  def some_method
  ^^^^^^^^^^^^^^^ Method `A#some_method` is defined at both (string):2 and (string):5.
    implement 2
  end
end
