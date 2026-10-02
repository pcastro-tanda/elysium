foo do
  it
  Class.new do
    def self.name
      'Foo'
    end
  end
end

bar do
  it
  Class.new do
    def self.name
      'Bar'
    end
  end
end
