class Person
  include AnotherModule
  extend SomeModule

  CustomError = Class.new(StandardError)

  validates :name

  attr_reader :name
  ^^^^^^^^^^^^^^^^^ `attribute_macros` is supposed to appear before `macros`.

  def self.some_public_class_method
  end

  def initialize
  end

  def some_public_method
  end


  def yet_other_public_method
  end

  protected

  def some_protected_method
  end

  def other_public_method
  end

  private :other_public_method

  private

  def some_private_method
  end
end
