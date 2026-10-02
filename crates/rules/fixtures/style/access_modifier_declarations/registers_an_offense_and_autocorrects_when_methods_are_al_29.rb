class << self
  def bar; end
  def baz; end

  # comment
  private :bar, :baz
  ^^^^^^^ `private` should not be inlined in method definitions.
end
