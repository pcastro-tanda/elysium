module A
  def self.some_method
    implement 1
  end
end
module A
  def self.some_method
  ^^^^^^^^^^^^^^^^^^^^ Method `A.some_method` is defined at both test.rb:2 and test.rb:7.
    implement 2
  end
end
