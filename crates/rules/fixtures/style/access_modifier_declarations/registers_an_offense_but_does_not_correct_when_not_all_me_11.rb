module Foo
  def bar; end

  protected :bar, :baz
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
