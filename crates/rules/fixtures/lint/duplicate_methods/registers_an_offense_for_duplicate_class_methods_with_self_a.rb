class A
  def self.some_method
    implement 1
  end
  def A.some_method
  ^^^^^^^^^^^^^^^^^ Method `A.some_method` is defined at both src.rb:2 and src.rb:5.
    implement 2
  end
end
