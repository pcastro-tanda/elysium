A = Class.new do
  def some_method
    implement 1
  end
end
A = Class.new do
  def some_method
  ^^^^^^^^^^^^^^^ Method `A#some_method` is defined at both dups.rb:2 and dups.rb:7.
    implement 2
  end
end
