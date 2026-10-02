Hash[h].filter { |x| x.match? /regexp/ }
Hash[:foo, 0, :bar, 1].filter { |x| x.match? /regexp/ }
