module Foo
  def bar; end
  def baz; end

  public :bar, :baz
  ^^^^^^ `public` should not be inlined in method definitions.
end
