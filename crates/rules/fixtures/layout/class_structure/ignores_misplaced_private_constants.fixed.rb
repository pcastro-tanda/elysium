class Foo
  PUBLIC_CONST = 'public'
  def name; end

  PRIVATE_CONST1 = 1
  PRIVATE_CONST2 = 2
  private_constant :PRIVATE_CONST1, :PRIVATE_CONST2
end
