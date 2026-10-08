Hash[h].reject { |x| x.is_a?(Foo) }
Hash[:foo, 0, :bar, 1].reject { |x| x.is_a?(Foo) }
