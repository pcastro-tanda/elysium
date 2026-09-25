class SomeClass
  def some_method
    puts 10
  end
  private
  ^^^^^^^ Useless `private` access modifier.
  def self.some_method
    puts 10
  end
end
