module A
  class << self
    def method1
    end
    protected
    ^^^^^^^^^ Useless `protected` access modifier.
  end
end
