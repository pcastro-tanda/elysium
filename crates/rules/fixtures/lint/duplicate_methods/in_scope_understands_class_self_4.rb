A = Module.new do
  class << self
    def some_method
      implement 1
    end
    def some_method
    ^^^^^^^^^^^^^^^ Method `A.some_method` is defined at both test.rb:3 and test.rb:6.
      implement 2
    end
  end
end
