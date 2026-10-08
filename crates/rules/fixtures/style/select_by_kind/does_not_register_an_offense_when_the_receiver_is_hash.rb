Hash[h].select { |x| x.is_a?(Foo) }
Hash[:foo, 0, :bar, 1].select { |x| x.is_a?(Foo) }
