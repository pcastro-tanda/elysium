class A
  private

  attr_accessor :foo

  def initialize
  ^^^^^^^^^^^^^^ `initializer` is supposed to appear before `private_attribute_macros`.
  end
end
