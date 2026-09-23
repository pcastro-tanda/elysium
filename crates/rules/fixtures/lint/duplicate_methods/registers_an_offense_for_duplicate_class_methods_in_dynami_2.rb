A = Module.new do
  def self.some_method
    implement 1
  end
  def self.some_method
  ^^^^^^^^^^^^^^^^^^^^ Method `A.some_method` is defined at both dups.rb:2 and dups.rb:5.
    implement 2
  end
end
