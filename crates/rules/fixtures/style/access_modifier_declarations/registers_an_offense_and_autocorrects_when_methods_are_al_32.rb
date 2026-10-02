module Foo
  def bar; end
  def baz; end

  # comment
  protected :bar, :baz
  ^^^^^^^^^ `protected` should not be inlined in method definitions.
end
