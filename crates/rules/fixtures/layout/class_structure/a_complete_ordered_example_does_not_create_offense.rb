class Person
  # extend and include go first
  extend SomeModule
  include AnotherModule

  # inner classes
  CustomError = Class.new(StandardError)

  # constants are next
  SOME_CONSTANT = 20

  # afterwards we have attribute macros
  attr_reader :name

  # then we have public delegate macros
  delegate :to_s, to: :name

  # followed by other macros (if any)
  validates :name

  # public class methods are next in line
  def self.some_method
  end

  # initialization goes between class methods and other instance methods
  def initialize
  end

  # followed by other public instance methods
  def some_method
  end

  # protected attribute macros and methods go next
  protected

  attr_reader :protected_name

  def some_protected_method
  end

  # private attribute macros, delegate macros and methods are grouped near the end
  private

  attr_reader :private_name

  delegate :some_private_delegate, to: :name

  def some_private_method
  end
end
