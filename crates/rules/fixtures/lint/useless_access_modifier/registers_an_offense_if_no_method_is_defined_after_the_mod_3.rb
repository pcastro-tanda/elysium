module A
  class << self
    def method1
    end
    private
    ^^^^^^^ Useless `private` access modifier.
  end
end
