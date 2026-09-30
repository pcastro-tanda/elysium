class << self
  def bar; end
  def baz; end

  protected :bar, :baz
  ^^^^^^^^^ `protected` should not be inlined in method definitions.

  protected
  def quux; end
end
