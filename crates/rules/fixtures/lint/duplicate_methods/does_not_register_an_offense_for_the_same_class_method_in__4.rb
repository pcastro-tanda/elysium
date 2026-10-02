foo do
  a = bar
  Class.new do
    def self.name
      'Foo'
    end
  end
end

foo do
  b = baz
  Class.new do
    def self.name
      'Bar'
    end
  end
end
