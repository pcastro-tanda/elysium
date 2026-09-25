module A
  def method1
  end
  class << self
    private
    ^^^^^^^ Useless `private` access modifier.
  end
end
