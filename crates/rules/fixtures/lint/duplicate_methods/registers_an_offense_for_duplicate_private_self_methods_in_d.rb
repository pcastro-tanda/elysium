A = Class.new do
  private def self.some_method
    implement 1
  end
  private def self.some_method
          ^^^^^^^^^^^^^^^^^^^^ Method `A.some_method` is defined at both (string):2 and (string):5.
    implement 2
  end
end
