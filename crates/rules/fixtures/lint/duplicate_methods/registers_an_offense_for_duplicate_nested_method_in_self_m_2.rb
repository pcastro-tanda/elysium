module A
  def self.foo
    def some_method
      implement 1
    end
  end

  def self.foo
  ^^^^^^^^^^^^ Method `A.foo` is defined at both example.rb:2 and example.rb:8.
    def some_method
    ^^^^^^^^^^^^^^^ Method `A#some_method` is defined at both example.rb:3 and example.rb:9.
      implement 2
    end
  end
end
