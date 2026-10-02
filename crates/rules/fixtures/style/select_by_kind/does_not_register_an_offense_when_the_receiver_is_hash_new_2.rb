Hash&.new.select { |x| x.is_a?(Foo) }
Hash&.new { |hash, key| :default }.select { |x| x.is_a?(Foo) }
