module Foo
  def bar; end

  public :bar, :baz
  ^^^^^^ `public` should not be inlined in method definitions.
end
