Class.new do
  private def some_method
    implement 1
  end
  private def some_method
          ^^^^^^^^^^^^^^^ Method `Object#some_method` is defined at both (string):2 and (string):5.
    implement 2
  end
end
