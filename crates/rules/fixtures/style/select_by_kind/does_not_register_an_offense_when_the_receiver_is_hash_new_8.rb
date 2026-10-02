Hash&.new.reject { |x| x.is_a?(Foo) }
Hash&.new { |hash, key| :default }.reject { |x| x.is_a?(Foo) }
