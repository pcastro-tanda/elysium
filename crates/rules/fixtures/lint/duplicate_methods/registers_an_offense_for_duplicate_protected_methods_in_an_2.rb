Module.new do
  protected def some_method
    implement 1
  end
  protected def some_method
            ^^^^^^^^^^^^^^^ Method `Object#some_method` is defined at both (string):2 and (string):5.
    implement 2
  end
end
