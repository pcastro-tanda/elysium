Hash[h].find_all { |x| x.match? /regexp/ }
Hash[:foo, 0, :bar, 1].find_all { |x| x.match? /regexp/ }
