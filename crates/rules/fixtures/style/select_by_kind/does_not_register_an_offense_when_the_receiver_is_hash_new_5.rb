Hash.new.filter { |x| x.is_a?(Foo) }
Hash.new(:default).filter { |x| x.is_a?(Foo) }
Hash.new { |hash, key| :default }.filter { |x| x.is_a?(Foo) }
