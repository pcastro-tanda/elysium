def build_foo
  Class.new do
    def self.name
      'Foo'
    end
  end
end
def build_bar
  Class.new do
    def self.name
      'Bar'
    end
  end
end
