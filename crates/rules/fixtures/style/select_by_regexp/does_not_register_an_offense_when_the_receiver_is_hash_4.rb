Hash[h].reject { |x| x.match? /regexp/ }
Hash[:foo, 0, :bar, 1].reject { |x| x.match? /regexp/ }
