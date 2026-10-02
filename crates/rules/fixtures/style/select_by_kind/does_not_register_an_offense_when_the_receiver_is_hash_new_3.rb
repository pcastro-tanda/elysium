Hash.new.find_all { |x| x.is_a?(Foo) }
Hash.new(:default).find_all { |x| x.is_a?(Foo) }
Hash.new { |hash, key| :default }.find_all { |x| x.is_a?(Foo) }
