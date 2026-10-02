class Foo
  def name; end

  PRIVATE_CONST1 = 1
  PRIVATE_CONST2 = 2
  private_constant :PRIVATE_CONST1, :PRIVATE_CONST2
  PUBLIC_CONST = 'public'
  ^^^^^^^^^^^^^^^^^^^^^^^ `constants` is supposed to appear before `public_methods`.
end
