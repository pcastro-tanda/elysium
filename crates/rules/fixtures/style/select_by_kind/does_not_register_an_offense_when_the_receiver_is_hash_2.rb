Hash[h].find_all { |x| x.is_a?(Foo) }
Hash[:foo, 0, :bar, 1].find_all { |x| x.is_a?(Foo) }
