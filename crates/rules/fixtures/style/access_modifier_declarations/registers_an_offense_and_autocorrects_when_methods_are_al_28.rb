class << self
  def bar; end
  def baz; end

  private :bar, :baz
  ^^^^^^^ `private` should not be inlined in method definitions.
end
