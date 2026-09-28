module Foo
  def bar; end

  private :bar, :baz
  ^^^^^^^ `private` should not be inlined in method definitions.
end
