Hash[h].select { |x| x.match? /regexp/ }
Hash[:foo, 0, :bar, 1].select { |x| x.match? /regexp/ }
