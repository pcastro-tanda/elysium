n.each do |x|
  do_something
  x.collect { |a| a[:foo] }
end
