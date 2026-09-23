module B
  A = Class.new do
    def some_method
      implement 1
    end
    def some_method
    ^^^^^^^^^^^^^^^ Method `B::A#some_method` is defined at both test.rb:3 and test.rb:6.
      implement 2
    end
    def self.another
    end
    def self.another
    ^^^^^^^^^^^^^^^^ Method `B::A.another` is defined at both test.rb:9 and test.rb:11.
    end
  end
end
