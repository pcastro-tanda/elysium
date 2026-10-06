n.each do
  do_something
  _1.collect { |a| a[:foo] }
end
