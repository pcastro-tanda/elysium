class A
  def method1
  end
  class << self
    protected
    ^^^^^^^^^ Useless `protected` access modifier.
  end
end
