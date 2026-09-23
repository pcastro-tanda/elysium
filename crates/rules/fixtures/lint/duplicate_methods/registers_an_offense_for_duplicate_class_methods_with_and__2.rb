module A
  class << self
    def some_method
      implement 1
    end
  end
  def A.some_method
  ^^^^^^^^^^^^^^^^^ Method `A.some_method` is defined at both test.rb:3 and test.rb:7.
    implement 2
  end
end
